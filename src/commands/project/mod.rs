//! What `nvm init` and `nvm upgrade` share: the releases to choose from, the
//! policy's say on them, and writing the `.nvmrc` and `package.json`.

mod candidates;
mod stop;
mod write;

pub use candidates::{
    Candidate, Exact, exact, first_allowed, in_major, majors, newest_allowed, node_candidates,
    tool_candidates,
};
pub use stop::Stop;
pub use write::{PackageEdit, ProjectFiles, current_nvmrc, new_nvmrc, new_package_json, save};
