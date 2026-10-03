use std::ffi::OsString;
use std::io;
use std::path::PathBuf;

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

    fn current_dir(&self) -> io::Result<PathBuf> {
        std::env::current_dir()
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

    #[test]
    fn current_dir_is_the_directory_of_the_process() {
        assert_eq!(
            StdEnv.current_dir().unwrap(),
            std::env::current_dir().unwrap()
        );
    }
}
