//! What `nvm` knows about `npm` without running it: which `npm` a `node` can
//! upgrade to, the `default-packages` file, and the output of `npm list -g`.

pub mod default_packages;
pub mod global_packages;
pub mod upgrade;
