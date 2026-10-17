//! The policy flags, which can stand before the subcommand or after it.

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

/// The subcommands whose later arguments belong to the program they run.
const CHILD_COMMANDS: [&str; 2] = ["exec", "run"];

type Input<I> = std::iter::Peekable<I>;

/// Takes the policy flags, in `--flag value` and `--flag=value` form, from
/// before the subcommand and from anywhere after it up to a `--`. After
/// `exec` and `run` only the flags right after the subcommand are taken: a
/// policy flag further on belongs to the program they run and stays.
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
    let subcommand = input.next();
    let child_owns_the_rest = subcommand
        .as_ref()
        .is_some_and(|command| CHILD_COMMANDS.iter().any(|name| command == name));
    rest.extend(subcommand);
    take_flags(&mut input, &mut flags)?;
    if child_owns_the_rest {
        rest.extend(input);
    } else {
        take_flags_anywhere(&mut input, &mut flags, &mut rest)?;
    }
    Ok((flags, rest))
}

/// Takes every policy flag up to a `--`; the `--` and what follows stay.
fn take_flags_anywhere(
    input: &mut Input<impl Iterator<Item = OsString>>,
    flags: &mut PolicyFlags,
    rest: &mut Vec<OsString>,
) -> Result<(), FlagError> {
    while let Some(argument) = input.next() {
        let ends_the_options = argument == "--";
        rest.push(argument);
        if ends_the_options {
            rest.extend(input.by_ref());
            break;
        }
        take_flags(input, flags)?;
    }
    Ok(())
}

fn take_flags(
    input: &mut Input<impl Iterator<Item = OsString>>,
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
        if value.trim().is_empty() {
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
