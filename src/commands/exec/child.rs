//! The environment `nvm-exec` gives the command: what its `nvm use
//! "$NODE_VERSION"` exports, plus `NODE_VERSION`, `NVM_CD_FLAGS` (empty, as
//! sourcing nvm.sh leaves it) and `NVM_DIR`.

use crate::commands::deactivate::changes;
use crate::commands::transcript::Transcript;
use crate::commands::use_version::apply::Switch;
use crate::context::Context;
use crate::error::CliError;
use crate::ports::Invocation;

/// Variables to set (in order) and to take out.
#[derive(Debug, Default)]
pub(crate) struct Environment {
    set: Vec<(String, String)>,
    removed: Vec<String>,
}

impl Environment {
    fn set(mut self, name: &str, value: &str) -> Self {
        self.set.push((name.to_owned(), value.to_owned()));
        self
    }

    fn remove(mut self, name: &str) -> Self {
        self.removed.push(name.to_owned());
        self
    }

    /// Leaves `NODE_VERSION` as the parent has it (what `nvm-exec` does: it
    /// never sets it).
    pub(crate) fn keep_node_version(mut self) -> Self {
        self.set.retain(|(name, _)| name != "NODE_VERSION");
        self
    }

    fn exec_variables(self, context: &Context<'_>, node_version: &str) -> Result<Self, CliError> {
        let nvm_dir = context.nvm_dir()?;
        Ok(self
            .set("NODE_VERSION", node_version)
            .set("NVM_CD_FLAGS", "")
            .set("NVM_DIR", &nvm_dir.to_string_lossy()))
    }
}

/// An installed version: `PATH` (already switched), `MANPATH` when a
/// `manpath` program is on it, `NVM_BIN` and `NVM_INC`.
pub(crate) fn installed(
    context: &Context<'_>,
    switch: &Switch,
    path: &str,
    node_version: &str,
) -> Result<Environment, CliError> {
    let mut environment = Environment::default().set("PATH", path);
    if let Some(manpath) = switch.manpath(context, path)? {
        environment = environment.set("MANPATH", &manpath);
    }
    environment
        .set("NVM_BIN", &switch.bin())
        .set("NVM_INC", &switch.include())
        .exec_variables(context, node_version)
}

/// `system`: what a silent `nvm deactivate` changes, and the new `PATH`.
pub(crate) fn system(context: &Context<'_>) -> Result<(String, Environment), CliError> {
    let mut path = context.text_var("PATH")?;
    let mut environment = Environment::default();
    for change in changes(context, true, &mut Transcript::default())? {
        environment = match change.value {
            Some(value) => {
                if change.name == "PATH" {
                    path.clone_from(&value);
                }
                environment.set(change.name, &value)
            }
            None => environment.remove(change.name),
        };
    }
    Ok((path, environment.exec_variables(context, "system")?))
}

/// The command (a leading `--` dropped, as the shell's `exec` does) with
/// `environment`, or `None` when there is no command.
pub(crate) fn invocation(command: &[String], environment: Environment) -> Option<Invocation> {
    let command = match command.split_first() {
        Some((first, rest)) if first == "--" => rest,
        _ => command,
    };
    let (program, args) = command.split_first()?;
    let mut invocation = Invocation::new(program);
    invocation.args = args.to_vec();
    invocation.env = environment.set;
    invocation.env_remove = environment.removed;
    Some(invocation)
}
