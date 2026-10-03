//! The fish rendering of a [`Script`](super::Script).
//!
//! `export NAME=value` becomes `set -gx NAME 'value'`, `unset NAME` becomes
//! `set -e NAME`, and the command-hash reset is dropped: fish keeps no hash
//! of command locations. A variable whose name ends in `PATH` (`PATH`,
//! `MANPATH`, `NODE_PATH`) is a path variable in fish, which splits the one
//! colon-joined value on `:` and joins it back when exporting it, keeping
//! empty entries and a trailing colon: the programs fish starts see exactly
//! the value nvm.sh would have exported.

use super::Step;

/// Single-quotes `value` for fish, where only `\` and `'` are special
/// inside single quotes (`\` becomes `\\` and `'` becomes `\'`).
#[must_use]
pub fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\\', r"\\").replace('\'', r"\'"))
}

pub(super) fn render(step: &Step) -> String {
    match step {
        Step::Export(name, value) => format!("set -gx {name} {}\n", quote(value)),
        Step::Unset(name) => format!("set -e {name}\n"),
        Step::HashReset => String::new(),
    }
}
