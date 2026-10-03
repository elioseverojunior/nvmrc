use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use crate::ports::{Process, ProcessOutput};

/// Programs by path: each prints a fixed output, or fails when it has none.
#[derive(Default)]
pub struct FakeProcess {
    outputs: BTreeMap<PathBuf, ProcessOutput>,
}

impl FakeProcess {
    #[must_use]
    pub fn with_output(mut self, program: &str, stdout: &str) -> Self {
        let output = ProcessOutput {
            success: true,
            stdout: stdout.to_owned(),
        };
        self.outputs.insert(PathBuf::from(program), output);
        self
    }

    #[must_use]
    pub fn with_failure(mut self, program: &str) -> Self {
        let output = ProcessOutput {
            success: false,
            stdout: String::new(),
        };
        self.outputs.insert(PathBuf::from(program), output);
        self
    }
}

impl Process for FakeProcess {
    fn run(&self, program: &Path, _args: &[&str]) -> io::Result<ProcessOutput> {
        self.outputs
            .get(program)
            .cloned()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_process_answers_by_program_path() {
        let process = FakeProcess::default()
            .with_output("/usr/bin/node", "v22.1.0\n")
            .with_failure("/usr/bin/broken");
        let ok = process
            .run(Path::new("/usr/bin/node"), &["--version"])
            .unwrap();
        assert_eq!((ok.success, ok.stdout.as_str()), (true, "v22.1.0\n"));
        let failed = process.run(Path::new("/usr/bin/broken"), &[]).unwrap();
        assert!(!failed.success);
        assert!(process.run(Path::new("/missing"), &[]).is_err());
    }
}
