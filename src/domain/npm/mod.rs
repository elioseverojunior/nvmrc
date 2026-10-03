//! What `nvm` knows about `npm` without running it: which `npm` a `node` can
//! upgrade to, the `default-packages` file, the output of `npm list -g`, and
//! the npm prefix settings that break nvm.

pub mod default_packages;
pub mod global_packages;
pub mod prefix;
pub mod upgrade;
