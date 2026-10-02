use std::io;
use std::path::Path;

use crate::ports::{Process, ProcessOutput};

/// The `Process` of a `Context` that was not given one: it runs nothing.
pub struct NoProcess;

impl Process for NoProcess {
    fn run(&self, _program: &Path, _args: &[&str]) -> io::Result<ProcessOutput> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_nothing() {
        let error = NoProcess.run(Path::new("/bin/true"), &[]).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    }
}
