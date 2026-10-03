use std::io;
use std::path::Path;

use crate::ports::{Completed, Invocation, Process, ProcessOutput};

/// The `Process` of a `Context` that was not given one: it runs nothing.
pub struct NoProcess;

impl Process for NoProcess {
    fn execute(&self, _invocation: &Invocation) -> io::Result<Completed> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

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
        let error = NoProcess
            .execute(&Invocation::new("/bin/true"))
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    }
}
