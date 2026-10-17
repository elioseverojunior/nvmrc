//! The `NVM_AUTH_HEADER` (and `NVMRC_NPM_AUTH_HEADER`) value, as `nvm.sh`
//! sanitizes it before sending it as an `Authorization` header.

use crate::ports::Env;

/// Keeps only letters, digits, space and `: _ . + / = ~ -`: the full base64
/// and base64url alphabets plus what `Basic <token>` and `Bearer <token>`
/// need. Everything else, such as a newline that could smuggle a second
/// header, is dropped.
#[must_use]
pub fn sanitize_auth_header(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || " :_.+/=~-".contains(*character))
        .collect()
}

/// The variable `name` sanitized, as `nvm.sh` reads `NVM_AUTH_HEADER`; `None`
/// when it is unset or empty.
#[must_use]
pub fn auth_header_from(env: &dyn Env, name: &str) -> Option<String> {
    env.var(name)
        .filter(|value| !value.is_empty())
        .map(|value| sanitize_auth_header(&value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::FakeEnv;

    #[test]
    fn keeps_basic_and_bearer_tokens_intact() {
        assert_eq!(
            sanitize_auth_header("Basic dXNlcjpwYXNz+/=="),
            "Basic dXNlcjpwYXNz+/=="
        );
        assert_eq!(sanitize_auth_header("Bearer a.b-c_d~e"), "Bearer a.b-c_d~e");
    }

    #[test]
    fn drops_anything_that_could_inject_a_header() {
        assert_eq!(
            sanitize_auth_header("Bearer x\r\nX-Evil: 1"),
            "Bearer xX-Evil: 1"
        );
        assert_eq!(sanitize_auth_header("a;b'c\"d`e$f"), "abcdef");
    }

    #[test]
    fn auth_header_from_ignores_a_blank_value() {
        let env = FakeEnv::default()
            .with_var("BLANK", "")
            .with_var("TOKEN", "Bearer x\r\ny");
        assert_eq!(auth_header_from(&env, "BLANK"), None);
        assert_eq!(auth_header_from(&env, "UNSET"), None);
        assert_eq!(
            auth_header_from(&env, "TOKEN").as_deref(),
            Some("Bearer xy")
        );
    }
}
