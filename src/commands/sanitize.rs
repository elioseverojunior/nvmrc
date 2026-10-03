//! `nvm_sanitize_path`: a path shown to the user, with `$NVM_DIR` and then
//! `$HOME` written as the variables.

use crate::context::Context;

/// Replaces every `$NVM_DIR` in `path` with the text `${NVM_DIR}`, then every
/// `$HOME` with `${HOME}`. A path that is exactly one of them is left as it
/// is for that variable, as nvm.sh does; an unset or empty variable is
/// skipped.
#[must_use]
pub fn sanitize_path(context: &Context<'_>, path: &str) -> String {
    let nvm_dir = context
        .nvm_dir()
        .ok()
        .map(|dir| dir.to_string_lossy().into_owned());
    let home = context.env.var("HOME");
    [(nvm_dir, "${NVM_DIR}"), (home, "${HOME}")]
        .into_iter()
        .fold(path.to_owned(), |text, (value, name)| match value {
            Some(value) if !value.is_empty() && text != value => text.replace(&value, name),
            _ => text,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::{FakeEnv, FakeFileSystem};

    fn sanitized(env: &FakeEnv, path: &str) -> String {
        let fs = FakeFileSystem::default();
        sanitize_path(&Context::new(&fs, env), path)
    }

    #[test]
    fn nvm_dir_then_home_become_the_variables() {
        let env = FakeEnv::default()
            .with_var("HOME", "/h")
            .with_var("NVM_DIR", "/h/.nvm");
        assert_eq!(sanitized(&env, "/h/.nvm/v/etc"), "${NVM_DIR}/v/etc");
        assert_eq!(sanitized(&env, "/h/.npmrc"), "${HOME}/.npmrc");
        assert_eq!(sanitized(&env, "/elsewhere"), "/elsewhere");
    }

    #[test]
    fn a_path_equal_to_a_variable_keeps_it_unless_the_other_applies() {
        let env = FakeEnv::default()
            .with_var("HOME", "/h")
            .with_var("NVM_DIR", "/h/.nvm");
        assert_eq!(sanitized(&env, "/h/.nvm"), "${HOME}/.nvm");
        assert_eq!(sanitized(&env, "/h"), "/h");
    }

    #[test]
    fn an_unset_home_is_skipped() {
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        assert_eq!(sanitized(&env, "/n/x"), "${NVM_DIR}/x");
    }
}
