//! In-memory implementations of the ports, for unit tests only.

mod archive;
mod channel;
mod cpu;
mod digest;
mod env;
mod file_system;
mod http;
mod process;
mod sleeper;

pub use archive::FakeArchive;
pub use channel::FakeScriptChannel;
pub use cpu::FakeCpu;
pub use digest::FakeDigest;
pub use env::FakeEnv;
pub use file_system::FakeFileSystem;
pub use http::FakeHttp;
pub use process::FakeProcess;
pub use sleeper::FakeSleeper;
