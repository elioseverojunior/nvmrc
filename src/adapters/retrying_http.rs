//! Retries transient download failures with exponential backoff.

use std::time::Duration;

use crate::ports::{Http, HttpError, Sleeper};

/// Wraps an [`Http`]: a network error, a 5xx or a 429 is tried again after
/// `base_delay`, then twice as long, and so on, up to `attempts` tries in all.
/// Any other status (a 404, say) and an unusable body are final at once.
pub struct RetryingHttp<'a> {
    inner: &'a dyn Http,
    sleeper: &'a dyn Sleeper,
    attempts: u32,
    base_delay: Duration,
}

impl<'a> RetryingHttp<'a> {
    #[must_use]
    pub fn new(inner: &'a dyn Http, sleeper: &'a dyn Sleeper) -> Self {
        Self {
            inner,
            sleeper,
            attempts: 3,
            base_delay: Duration::from_millis(250),
        }
    }

    #[must_use]
    pub fn with_policy(mut self, attempts: u32, base_delay: Duration) -> Self {
        self.attempts = attempts.max(1);
        self.base_delay = base_delay;
        self
    }
}

fn is_transient(error: &HttpError) -> bool {
    match error {
        HttpError::Transport { .. } => true,
        HttpError::Status { code, .. } => *code >= 500 || *code == 429,
        HttpError::Body { .. } => false,
    }
}

impl RetryingHttp<'_> {
    fn retrying<T>(&self, request: impl Fn() -> Result<T, HttpError>) -> Result<T, HttpError> {
        let mut delay = self.base_delay;
        let mut attempt = 1;
        loop {
            match request() {
                Err(error) if is_transient(&error) && attempt < self.attempts => {
                    self.sleeper.sleep(delay);
                    delay = delay.saturating_mul(2);
                    attempt += 1;
                }
                result => return result,
            }
        }
    }
}

impl Http for RetryingHttp<'_> {
    fn get_text(&self, url: &str) -> Result<String, HttpError> {
        self.retrying(|| self.inner.get_text(url))
    }

    fn get_bytes(&self, url: &str) -> Result<Vec<u8>, HttpError> {
        self.retrying(|| self.inner.get_bytes(url))
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;

    use super::*;
    use crate::fakes::FakeSleeper;

    /// Answers from a script, one entry per request.
    struct Scripted(RefCell<VecDeque<Result<String, HttpError>>>);

    impl Scripted {
        fn new(script: Vec<Result<String, HttpError>>) -> Self {
            Self(RefCell::new(script.into()))
        }
    }

    impl Http for Scripted {
        fn get_text(&self, url: &str) -> Result<String, HttpError> {
            self.0.borrow_mut().pop_front().unwrap_or_else(|| {
                Err(HttpError::Transport {
                    url: url.to_owned(),
                    message: "script exhausted".to_owned(),
                })
            })
        }

        fn get_bytes(&self, url: &str) -> Result<Vec<u8>, HttpError> {
            self.get_text(url).map(String::into_bytes)
        }
    }

    fn status(code: u16) -> Result<String, HttpError> {
        Err(HttpError::Status {
            url: "u".to_owned(),
            code,
        })
    }

    fn fetch(script: Vec<Result<String, HttpError>>) -> (Result<String, HttpError>, Vec<Duration>) {
        let inner = Scripted::new(script);
        let sleeper = FakeSleeper::default();
        let result = RetryingHttp::new(&inner, &sleeper).get_text("u");
        (result, sleeper.slept())
    }

    #[test]
    fn bytes_are_retried_like_text() {
        let inner = Scripted::new(vec![status(502), Ok("tar".to_owned())]);
        let sleeper = FakeSleeper::default();
        let body = RetryingHttp::new(&inner, &sleeper).get_bytes("u");
        assert_eq!(body.unwrap(), b"tar");
        assert_eq!(sleeper.slept(), [Duration::from_millis(250)]);
    }

    #[test]
    fn a_first_try_that_works_never_sleeps() {
        let (result, slept) = fetch(vec![Ok("body".to_owned())]);
        assert_eq!(result.unwrap(), "body");
        assert!(slept.is_empty());
    }

    #[test]
    fn transient_failures_are_retried_with_doubling_delays() {
        let (result, slept) = fetch(vec![status(503), status(500), Ok("body".to_owned())]);
        assert_eq!(result.unwrap(), "body");
        assert_eq!(
            slept,
            [Duration::from_millis(250), Duration::from_millis(500)]
        );
    }

    #[test]
    fn network_errors_and_429_are_transient_too() {
        let transport = Err(HttpError::Transport {
            url: "u".to_owned(),
            message: "reset".to_owned(),
        });
        let (result, slept) = fetch(vec![transport, status(429), Ok("x".to_owned())]);
        assert!(result.is_ok());
        assert_eq!(slept.len(), 2);
    }

    #[test]
    fn it_gives_up_after_the_last_attempt_with_the_last_error() {
        let (result, slept) = fetch(vec![status(503), status(503), status(502)]);
        assert_eq!(
            result.unwrap_err(),
            HttpError::Status {
                url: "u".into(),
                code: 502
            }
        );
        assert_eq!(slept.len(), 2);
    }

    #[test]
    fn a_client_error_is_final_at_once() {
        let (result, slept) = fetch(vec![status(404), Ok("never".to_owned())]);
        assert_eq!(
            result.unwrap_err(),
            HttpError::Status {
                url: "u".into(),
                code: 404
            }
        );
        assert!(slept.is_empty());
    }

    #[test]
    fn an_unusable_body_is_final_at_once() {
        let body = Err(HttpError::Body {
            url: "u".to_owned(),
            message: "too big".to_owned(),
        });
        let (result, slept) = fetch(vec![body, Ok("never".to_owned())]);
        assert!(matches!(result, Err(HttpError::Body { .. })));
        assert!(slept.is_empty());
    }

    #[test]
    fn the_delay_stops_growing_instead_of_overflowing() {
        let inner = Scripted::new(vec![status(503), status(503), status(503)]);
        let sleeper = FakeSleeper::default();
        let http = RetryingHttp::new(&inner, &sleeper).with_policy(3, Duration::MAX);
        assert!(http.get_text("u").is_err());
        assert_eq!(sleeper.slept(), [Duration::MAX, Duration::MAX]);
    }

    #[test]
    fn the_policy_can_be_changed_and_at_least_one_attempt_is_made() {
        let inner = Scripted::new(vec![status(503)]);
        let sleeper = FakeSleeper::default();
        let http = RetryingHttp::new(&inner, &sleeper).with_policy(0, Duration::from_secs(1));
        assert!(http.get_text("u").is_err());
        assert!(sleeper.slept().is_empty());
    }
}
