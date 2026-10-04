//! In-memory implementations of the ports, for unit tests only.

mod archive;
mod channel;
mod clock;
mod cpu;
mod digest;
mod env;
mod file_system;
mod http;
mod process;
mod prompt;
mod sleeper;
mod terminal;

pub use archive::FakeArchive;
pub use channel::FakeScriptChannel;
pub use clock::FakeClock;
pub use cpu::FakeCpu;
pub use digest::FakeDigest;
pub use env::FakeEnv;
pub use file_system::FakeFileSystem;
pub use http::FakeHttp;
pub use process::FakeProcess;
pub use prompt::FakePrompt;
pub use sleeper::FakeSleeper;
pub use terminal::FakeTerminal;
