use std::ffi::OsString;

use crate::ports::Env;

pub struct StdEnv;

impl Env for StdEnv {
    fn var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }

    fn var_os(&self, key: &str) -> Option<OsString> {
        std::env::var_os(key)
    }

    fn vars(&self) -> Vec<(String, String)> {
        std::env::vars_os()
            .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vars_lists_what_var_reads() {
        let vars = StdEnv.vars();
        assert!(!vars.is_empty());
        let (name, value) = &vars[0];
        assert_eq!(StdEnv.var(name).as_deref(), Some(value.as_str()));
    }
}
