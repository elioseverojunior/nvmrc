use std::io;
use std::path::Path;
use std::process::Command;

use crate::ports::{Process, ProcessOutput};

pub struct StdProcess;

impl Process for StdProcess {
    fn run(&self, program: &Path, args: &[&str]) -> io::Result<ProcessOutput> {
        let output = Command::new(program).args(args).output()?;
        Ok(ProcessOutput {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        })
    }
}
