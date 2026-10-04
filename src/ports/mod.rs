//! Traits through which the domain and commands reach the outside world.

mod archive;
mod channel;
mod env;
mod fs;
mod http;
mod process;
mod system;
mod terminal;

pub use archive::{Archive, Digest};
pub use channel::ScriptChannel;
pub use env::Env;
pub use fs::{DirEntry, FileInfo, FileSystem};
pub use http::{Http, HttpError};
pub use process::{Completed, Invocation, Process, ProcessOutput};
pub use system::{Clock, Cpu, Sleeper};
pub use terminal::{Prompt, Terminal};
