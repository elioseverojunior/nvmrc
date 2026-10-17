//! The real `Http`: a blocking client that follows redirects and speaks TLS.

use std::time::Duration;

use ureq::config::RedirectAuthHeaders;
use ureq::tls::{RootCerts, TlsConfig};

use crate::ports::{Http, HttpError};

/// How long a request may take. An index or `SHASUMS256.txt` is small, so
/// its whole call is bounded. An archive (25-45 MB) can take minutes on a
/// slow link, so, like curl in `nvm.sh`, its body has no time limit: only
/// connecting and waiting for the response headers are bounded.
#[derive(Clone, Copy)]
struct Timeouts {
    /// The whole of a `get_text` call, from the DNS lookup to the last byte.
    text_call: Duration,
    /// Connecting, and then waiting for the response headers, of any call.
    until_response: Duration,
}

const TIMEOUTS: Timeouts = Timeouts {
    text_call: Duration::from_secs(30),
    until_response: Duration::from_secs(30),
};
/// The bounds of a fetch on an automatic path (the shell startup), which must
/// never hold the shell for long: it warns and goes on instead.
const QUICK_TIMEOUTS: Timeouts = Timeouts {
    text_call: Duration::from_secs(5),
    until_response: Duration::from_secs(5),
};
const MAX_BODY_BYTES: u64 = 16 * 1024 * 1024;
/// An archive is far bigger than an index, but never this big.
const MAX_DOWNLOAD_BYTES: u64 = 1024 * 1024 * 1024;

/// The `Authorization` value and the host it belongs to: with a `host`, it is
/// sent only to that host (compared ASCII case-insensitively); without one,
/// on every request.
#[derive(Clone)]
pub struct AuthHeader {
    pub value: String,
    pub host: Option<String>,
}

impl AuthHeader {
    fn applies_to(&self, url: &str) -> bool {
        match &self.host {
            None => true,
            Some(host) => url_host(url).is_some_and(|target| target.eq_ignore_ascii_case(host)),
        }
    }
}

/// The host of `url`; `None` when it cannot be parsed or names none.
#[must_use]
pub fn url_host(url: &str) -> Option<String> {
    let uri: ureq::http::Uri = url.parse().ok()?;
    uri.host().map(str::to_owned)
}

pub struct UreqHttp {
    agent: ureq::Agent,
    auth_header: Option<AuthHeader>,
    text_call: Duration,
}

impl UreqHttp {
    /// `auth_header` is sent as `Authorization` when given, to the hosts it
    /// allows; sanitize it first (see `domain::http_header`).
    #[must_use]
    pub fn new(auth_header: Option<AuthHeader>) -> Self {
        Self::from_builder(ureq::Agent::config_builder(), TIMEOUTS, auth_header)
    }

    /// Like [`Self::new`], but bounded at a few seconds; meant to be used
    /// without retries, on the automatic paths.
    #[must_use]
    pub fn quick(auth_header: Option<AuthHeader>) -> Self {
        Self::from_builder(ureq::Agent::config_builder(), QUICK_TIMEOUTS, auth_header)
    }

    /// Like [`Self::new`], but ignoring `HTTP_PROXY` and friends, so a test
    /// reaches its local server whatever the environment says.
    #[cfg(test)]
    fn unproxied(auth_header: Option<AuthHeader>) -> Self {
        Self::unproxied_within(TIMEOUTS, auth_header)
    }

    /// Like [`Self::unproxied`], with `timeouts` instead of the real ones.
    #[cfg(test)]
    fn unproxied_within(timeouts: Timeouts, auth_header: Option<AuthHeader>) -> Self {
        let builder = ureq::Agent::config_builder().proxy(None);
        Self::from_builder(builder, timeouts, auth_header)
    }

    fn from_builder(
        builder: ureq::config::ConfigBuilder<ureq::typestate::AgentScope>,
        timeouts: Timeouts,
        auth_header: Option<AuthHeader>,
    ) -> Self {
        // Like curl: the credentials follow a redirect to the same host only.
        // Like curl: trust the system certificate store, so a corporate CA
        // (a TLS-inspecting proxy) is honoured.
        let tls = TlsConfig::builder()
            .root_certs(RootCerts::PlatformVerifier)
            .build();
        let config = builder
            .tls_config(tls)
            .timeout_connect(Some(timeouts.until_response))
            .timeout_recv_response(Some(timeouts.until_response))
            .redirect_auth_headers(RedirectAuthHeaders::SameHost)
            .build();
        Self {
            agent: config.into(),
            auth_header,
            text_call: timeouts.text_call,
        }
    }
}

fn transport(url: &str, message: impl ToString) -> HttpError {
    HttpError::Transport {
        url: url.to_owned(),
        message: message.to_string(),
    }
}

/// A body that is over the limit or not text is final; anything else that
/// goes wrong while reading it (a reset, say) is a transport error.
fn read_failure(url: &str, error: ureq::Error) -> HttpError {
    let unusable = match &error {
        ureq::Error::BodyExceedsLimit(_) => true,
        ureq::Error::Io(source) => source.kind() == std::io::ErrorKind::InvalidData,
        _ => false,
    };
    if unusable {
        HttpError::Body {
            url: url.to_owned(),
            message: error.to_string(),
        }
    } else {
        transport(url, error)
    }
}

impl UreqHttp {
    /// Sends the request and reads the body with `read`, held to `limits`.
    fn fetch<T>(
        &self,
        url: &str,
        limits: Limits,
        read: impl FnOnce(ureq::BodyWithConfig<'_>) -> Result<T, ureq::Error>,
    ) -> Result<T, HttpError> {
        // ureq follows a redirect from https to http unless `https_only` is
        // set, which it checks again on every hop.
        let mut request = self
            .agent
            .get(url)
            .config()
            .timeout_global(limits.whole_call)
            .https_only(limits.https_only)
            .build();
        if let Some(auth) = self
            .auth_header
            .as_ref()
            .filter(|auth| auth.applies_to(url))
        {
            request = request.header("Authorization", &auth.value);
        }
        match request.call() {
            Ok(mut response) => read(response.body_mut().with_config().limit(limits.bytes))
                .map_err(|error| read_failure(url, error)),
            Err(ureq::Error::StatusCode(code)) => Err(HttpError::Status {
                url: url.to_owned(),
                code,
            }),
            Err(error) => Err(transport(url, error)),
        }
    }
}

/// What one fetch is held to: a body size, a time for the whole call, and
/// whether every URL on the way must be https.
struct Limits {
    bytes: u64,
    whole_call: Option<Duration>,
    https_only: bool,
}

/// An archive: big, and with no time limit on its body.
fn download_limits(https_only: bool) -> Limits {
    Limits {
        bytes: MAX_DOWNLOAD_BYTES,
        whole_call: None,
        https_only,
    }
}

impl UreqHttp {
    fn text_limits(&self, https_only: bool) -> Limits {
        Limits {
            bytes: MAX_BODY_BYTES,
            whole_call: Some(self.text_call),
            https_only,
        }
    }
}

impl Http for UreqHttp {
    fn get_text(&self, url: &str) -> Result<String, HttpError> {
        self.fetch(url, self.text_limits(false), |body| body.read_to_string())
    }

    fn get_secure_text(&self, url: &str) -> Result<String, HttpError> {
        self.fetch(url, self.text_limits(true), |body| body.read_to_string())
    }

    fn get_bytes(&self, url: &str) -> Result<Vec<u8>, HttpError> {
        self.fetch(url, download_limits(false), |body| body.read_to_vec())
    }

    fn get_secure_bytes(&self, url: &str) -> Result<Vec<u8>, HttpError> {
        self.fetch(url, download_limits(true), |body| body.read_to_vec())
    }
}

#[cfg(test)]
mod tests;
