//! In-memory implementations of the ports, for unit tests only.

mod env;
mod file_system;
mod http;
mod process;

pub use env::FakeEnv;
pub use file_system::FakeFileSystem;
pub use http::FakeHttp;
pub use process::FakeProcess;
