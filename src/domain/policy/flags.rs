//! The policy flags, which can stand before or right after the subcommand.

use std::collections::BTreeMap;
use std::ffi::OsString;

/// A command-line flag that carries policy, and the variable that does too.
pub struct PolicyFlag {
    pub name: &'static str,
    pub variable: &'static str,
}

/// Every policy flag; later plans append to this table.
pub const POLICY_FLAGS: [PolicyFlag; 2] = [
    PolicyFlag {
        name: "--policy",
        variable: "NVMRC_POLICY",
    },
    PolicyFlag {
        name: "--min-node",
        variable: "NVM_MIN_VERSION",
    },
];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FlagError {
    #[error("nvm: {0} needs a value.")]
    MissingValue(String),
    #[error("nvm: {0} is given twice.")]
    Repeated(String),
    #[error("nvm: the value of {0} is not valid UTF-8.")]
    NotUtf8(String),
}

/// The values the policy flags were given, by flag name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PolicyFlags {
    values: BTreeMap<&'static str, String>,
}

impl PolicyFlags {
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    fn insert(&mut self, name: &'static str, value: String) -> Result<(), FlagError> {
        match self.values.insert(name, value) {
            Some(_) => Err(FlagError::Repeated(name.to_owned())),
            None => Ok(()),
        }
    }
}

/// Takes the policy flags from before the subcommand and from right after it,
/// in `--flag value` and `--flag=value` form; every other argument, including
/// a policy flag further on (it may belong to a program `exec` runs), stays.
///
/// # Errors
/// Returns a [`FlagError`] for a flag without a value, given twice, or whose
/// value is not UTF-8.
pub fn extract(args: Vec<OsString>) -> Result<(PolicyFlags, Vec<OsString>), FlagError> {
    let mut flags = PolicyFlags::default();
    let mut rest = Vec::with_capacity(args.len());
    let mut input = args.into_iter();
    rest.extend(input.next());
    let mut input = input.peekable();
    take_flags(&mut input, &mut flags)?;
    rest.extend(input.next());
    take_flags(&mut input, &mut flags)?;
    rest.extend(input);
    Ok((flags, rest))
}

fn take_flags(
    input: &mut std::iter::Peekable<impl Iterator<Item = OsString>>,
    flags: &mut PolicyFlags,
) -> Result<(), FlagError> {
    while let Some((flag, inline)) = input.peek().and_then(|argument| recognise(argument)) {
        input.next();
        let value = match inline {
            Some(value) => value,
            None => input
                .next()
                .ok_or_else(|| FlagError::MissingValue(flag.to_owned()))?
                .into_string()
                .map_err(|_| FlagError::NotUtf8(flag.to_owned()))?,
        };
        if value.is_empty() {
            return Err(FlagError::MissingValue(flag.to_owned()));
        }
        flags.insert(flag, value)?;
    }
    Ok(())
}

/// The flag `argument` names, and the value after `=` if it has one.
fn recognise(argument: &OsString) -> Option<(&'static str, Option<String>)> {
    let text = argument.to_str()?;
    POLICY_FLAGS.iter().find_map(|flag| {
        if text == flag.name {
            return Some((flag.name, None));
        }
        let value = text.strip_prefix(flag.name)?.strip_prefix('=')?;
        Some((flag.name, Some(value.to_owned())))
    })
}
