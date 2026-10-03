//! `NVM_INSTALL_THIRD_PARTY_HOOK`: a program that installs the version in
//! place of `nvm`, called as `<hook> <version> <flavor> std <method> <path>`.

use crate::commands::install::flow::{Halt, Step, Target};
use crate::commands::install::options::Options;
use crate::commands::install::place::is_valid_install;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::version::Flavor;
use crate::error::NvmExitCode;
use crate::ports::Invocation;

/// Runs the hook, and checks that it left a version behind.
///
/// # Errors
/// [`Halt`]: the hook's own status when it fails (1 when it could not run), 33
/// when it succeeds and installs nothing.
pub fn run(
    context: &Context<'_>,
    program: &str,
    options: &Options,
    target: &Target,
    transcript: &mut Transcript,
) -> Step<()> {
    transcript.err("** $NVM_INSTALL_THIRD_PARTY_HOOK env var set; dispatching to third-party installation method **");
    let flavor = match target.version.flavor {
        Flavor::Node => "node",
        Flavor::IoJs => "iojs",
    };
    let method = if options.no_binary {
        "source"
    } else {
        "binary"
    };
    let version = target.version.to_string();
    let path = target.path.display().to_string();
    let invocation = Invocation::new(program).args(&[&version, flavor, "std", method, &path]);
    let done = context.process().execute(&invocation);
    if let Ok(done) = &done {
        done.stdout.lines().for_each(|line| transcript.out(line));
        done.stderr.lines().for_each(|line| transcript.err(line));
    }
    if !done.as_ref().is_ok_and(|done| done.success) {
        transcript
            .err("*** Third-party $NVM_INSTALL_THIRD_PARTY_HOOK env var failed to install! ***");
        let code = done.ok().and_then(|done| done.code);
        return Err(Halt::Exit(NvmExitCode::passing_on(code)));
    }
    if !is_valid_install(context, &target.path) {
        transcript.err("*** Third-party $NVM_INSTALL_THIRD_PARTY_HOOK env var claimed to succeed, but failed to install! ***");
        return Err(Halt::Exit(NvmExitCode::HookClaimedSuccess));
    }
    Ok(())
}
