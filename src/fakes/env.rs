use std::collections::BTreeMap;
use std::ffi::OsString;

use crate::ports::Env;

#[derive(Default)]
pub struct FakeEnv {
    vars: BTreeMap<String, OsString>,
}

impl FakeEnv {
    #[must_use]
    pub fn with_var(self, key: &str, value: &str) -> Self {
        self.with_var_os(key, OsString::from(value))
    }

    #[must_use]
    pub fn with_var_os(mut self, key: &str, value: OsString) -> Self {
        self.vars.insert(key.to_owned(), value);
        self
    }
}

impl Env for FakeEnv {
    fn var(&self, key: &str) -> Option<String> {
        self.vars
            .get(key)
            .and_then(|value| value.to_str().map(str::to_owned))
    }

    fn var_os(&self, key: &str) -> Option<OsString> {
        self.vars.get(key).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_env_returns_the_variables_it_was_given() {
        let env = FakeEnv::default().with_var("HOME", "/home/me");
        assert_eq!(env.var("HOME"), Some("/home/me".to_owned()));
        assert_eq!(env.var_os("HOME"), Some(OsString::from("/home/me")));
        assert_eq!(env.var("MISSING"), None);
    }

    #[cfg(unix)]
    #[test]
    fn fake_env_keeps_non_utf8_values_for_var_os_only() {
        use std::os::unix::ffi::OsStringExt;
        let raw = OsString::from_vec(b"/n\xff".to_vec());
        let env = FakeEnv::default().with_var_os("NVM_DIR", raw.clone());
        assert_eq!(env.var("NVM_DIR"), None);
        assert_eq!(env.var_os("NVM_DIR"), Some(raw));
    }
}
