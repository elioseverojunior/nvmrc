//! `nvm upgrade [--node] [--npm] [--yarn] [--pnpm] [--major[=<N>]]`: moves
//! the tools pinned in the nearest `.nvmrc` to newer allowed releases (see
//! [`request`]).

mod request;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the upgrade run calls it in the next task")
)]
mod target;

use crate::commands::Output;
use crate::context::Context;
use crate::error::CliError;
use request::parse;

/// Upgrades the pinned tools named in `args`, or all of them.
///
/// # Errors
/// See [`request::parse`]; until the upgrade itself lands, a valid request is
/// [`CliError::Unsupported`].
pub fn run(_context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    parse(args)?;
    Err(CliError::Unsupported(
        "nvm upgrade is not available yet.".to_owned(),
    ))
}
