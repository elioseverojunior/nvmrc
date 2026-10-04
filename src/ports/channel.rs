use std::io;

/// Where the shell code of a command goes in the `nvm` function.
pub trait ScriptChannel {
    /// Hands `code` over to the calling shell.
    ///
    /// # Errors
    /// Propagates the underlying I/O error.
    fn send(&self, code: &str) -> io::Result<()>;
}
