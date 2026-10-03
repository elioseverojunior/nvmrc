//! The `NVM_AUTH_HEADER` value, as `nvm.sh` sanitizes it before sending it as
//! an `Authorization` header.

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
