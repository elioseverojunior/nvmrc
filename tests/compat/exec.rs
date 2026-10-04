//! `nvm exec`, `nvm run`, `nvm-exec` and the usage errors.

use super::Fixture::{File, Node};
use super::{Scenario, capture_all, check_all};

const SCENARIOS: &[Scenario] = &[
    Scenario::new(
        "X1 digit-named files are arguments",
        "fast/Running 'nvm exec' and 'nvm run' treat digit-named files as arguments, not versions",
    )
    .script(concat!(
        "nvm run 123.js </dev/null; echo rc=$?; nvm exec 1.2.3.js </dev/null; echo rc=$?; ",
        "nvm run v999.0.0-rc.1 app.js; echo rc=$?",
    ))
    .stdout("rc=1\nrc=1\nrc=1\n")
    .stderr(concat!(
        "No version provided and no .nvmrc file found\n",
        "WARNING: `nvm run` was invoked without a version argument and without an .nvmrc file.\n",
        "  Falling back to the active node version; this will become an error in a future release.\n",
        "  Pass `current` explicitly (e.g. `nvm run current ...`) to silence this warning.\n",
        "N/A: version \"none\" is not yet installed.\n",
        "\n",
        "You need to run `nvm install` to install and use the node version specified in `.nvmrc`.\n",
        "No version provided and no .nvmrc file found\n",
        "WARNING: `nvm exec` was invoked without a version argument and without an .nvmrc file.\n",
        "  Falling back to the active node version; this will become an error in a future release.\n",
        "  Pass `current` explicitly (e.g. `nvm exec current ...`) to silence this warning.\n",
        "N/A: version \"current\" is not yet installed.\n",
        "\n",
        "You need to run `nvm install current` to install and use it.\n",
        "N/A: version \"v999.0.0-rc.1\" is not yet installed.\n",
        "\n",
        "You need to run `nvm install v999.0.0-rc.1` to install and use it.\n",
    )),
    Scenario::new(
        "X2 exec without a version",
        "fast/Running 'nvm exec' and 'nvm run' without a resolvable version warn and fall back",
    )
    .script("nvm exec </dev/null; echo rc=$?; nvm exec --silent </dev/null; echo rc=$?")
    .stdout("rc=1\nrc=1\n")
    .stderr(concat!(
        "WARNING: `nvm exec` was invoked without a version argument and without an .nvmrc file.\n",
        "  Falling back to the active node version; this will become an error in a future release.\n",
        "  Pass `current` explicitly (e.g. `nvm exec current ...`) to silence this warning.\n",
        "N/A: version \"current\" is not yet installed.\n",
        "\n",
        "You need to run `nvm install current` to install and use it.\n",
        "N/A: version \"current\" is not yet installed.\n",
        "\n",
        "You need to run `nvm install current` to install and use it.\n",
    )),
    Scenario::new(
        "X3 run of an unknown version",
        "fast/Running 'nvm exec' and 'nvm run' without a resolvable version warn and fall back",
    )
    .script(concat!(
        "nvm run bogusversion </dev/null; echo rc=$?; ",
        "nvm run --silent bogusversion </dev/null; echo rc=$?",
    ))
    .stdout("rc=1\nrc=1\n")
    .stderr(concat!(
        "No version provided and no .nvmrc file found\n",
        "WARNING: `nvm run` was invoked without a version argument and without an .nvmrc file.\n",
        "  Falling back to the active node version; this will become an error in a future release.\n",
        "  Pass `current` explicitly (e.g. `nvm run current ...`) to silence this warning.\n",
        "N/A: version \"none\" is not yet installed.\n",
        "\n",
        "You need to run `nvm install` to install and use the node version specified in `.nvmrc`.\n",
        "N/A: version \"none\" is not yet installed.\n",
        "\n",
        "You need to run `nvm install` to install and use the node version specified in `.nvmrc`.\n",
    )),
    Scenario::new(
        "X4 focused usage errors",
        "fast/Subcommands with missing or invalid args show a focused usage error",
    )
    .script(concat!(
        "for c in \"cache bogus\" \"install\" \"run\" \"which\" \"uninstall\" \"uninstall a b\" ",
        "\"unalias\" \"unalias a b\" \"install-latest-npm extra\" \"reinstall-packages\" ",
        "\"copy-packages a b\"; do nvm $c </dev/null; echo \"rc=$? [$c]\"; done",
    ))
    .stdout(concat!(
        "rc=127 [cache bogus]\n",
        "rc=127 [install]\n",
        "rc=127 [run]\n",
        "rc=127 [which]\n",
        "rc=127 [uninstall]\n",
        "rc=127 [uninstall a b]\n",
        "rc=127 [unalias]\n",
        "rc=127 [unalias a b]\n",
        "rc=127 [install-latest-npm extra]\n",
        "rc=127 [reinstall-packages]\n",
        "rc=127 [copy-packages a b]\n",
    ))
    .stderr(concat!(
        "Usage: nvm cache dir\n",
        "       nvm cache clear\n",
        "  Run `nvm --help` for full help.\n",
        "No version provided and no .nvmrc file found\n",
        "Usage: nvm install [<version>]\n",
        "  Provide a <version>, or run from a directory containing an .nvmrc file.\n",
        "  Run `nvm --help` for full help.\n",
        "No version provided and no .nvmrc file found\n",
        "Usage: nvm run [<version>] [<args>]\n",
        "  Provide a <version>, or run from a directory containing an .nvmrc file.\n",
        "  Run `nvm --help` for full help.\n",
        "No version provided and no .nvmrc file found\n",
        "Usage: nvm which [current | <version>]\n",
        "  Provide a <version>, or run from a directory containing an .nvmrc file.\n",
        "  Run `nvm --help` for full help.\n",
        "Usage: nvm uninstall <version>\n",
        "       nvm uninstall --lts\n",
        "       nvm uninstall --lts=<LTS name>\n",
        "  Run `nvm --help` for full help.\n",
        "Usage: nvm uninstall <version>\n",
        "       nvm uninstall --lts\n",
        "       nvm uninstall --lts=<LTS name>\n",
        "  Run `nvm --help` for full help.\n",
        "Usage: nvm unalias <name>\n",
        "  Run `nvm --help` for full help.\n",
        "Usage: nvm unalias <name>\n",
        "  Run `nvm --help` for full help.\n",
        "Usage: nvm install-latest-npm\n",
        "  Run `nvm --help` for full help.\n",
        "Usage: nvm reinstall-packages <version>\n",
        "  Run `nvm --help` for full help.\n",
        "Usage: nvm copy-packages <version>\n",
        "  Run `nvm --help` for full help.\n",
    )),
    Scenario::new("X5 exec and run of an installed version", "nvm exec / nvm run (no upstream test)")
        .fixture(&[Node("v20.1.0")])
        .script("nvm exec 20 node; echo rc=$?; nvm run 20 --version; echo rc=$?")
        .stdout(concat!(
            "Running node v20.1.0\n",
            "v20.1.0\n",
            "rc=0\n",
            "Running node v20.1.0\n",
            "v20.1.0\n",
            "rc=0\n",
        )),
    Scenario::new("X6 exec of a missing command", "nvm-exec: exec \"$@\" (no upstream test)")
        .fixture(&[Node("v20.1.0")])
        .script("nvm exec 20 nonexistentcmd; echo rc=$?")
        .stdout("Running node v20.1.0\nrc=127\n")
        .stderr("nvm: nonexistentcmd: not found\n")
        .deviation("D15"),
    Scenario::new(
        "X7 nvm-exec with an .nvmrc version not installed",
        "fast/Running 'nvm-exec' should display required node version",
    )
    .fixture(&[File("proj/.nvmrc", "v0.42\n")])
    .script("\"$NVM_EXEC\" 2>&1; echo rc=$?")
    .stdout(concat!(
        "N/A: version \"v0.42\" is not yet installed.\n",
        "\n",
        "You need to run `nvm install v0.42` to install and use it.\n",
        "nvm-exec: unable to select a node version\n",
        "  Set `NODE_VERSION` (e.g. `NODE_VERSION=default`), or add an `.nvmrc` file.\n",
        "Found '<W>/proj/.nvmrc' with version <v0.42>\n",
        "rc=127\n",
    ))
    .deviation("D1"),
    Scenario::new(
        "X8 nvm-exec without a version",
        "fast/Running 'nvm-exec' should display required node version",
    )
    .script("\"$NVM_EXEC\" node; echo rc=$?")
    .stdout("rc=127\n")
    .stderr(concat!(
        "No version provided and no .nvmrc file found\n",
        "nvm-exec: unable to select a node version\n",
        "  Set `NODE_VERSION` (e.g. `NODE_VERSION=default`), or add an `.nvmrc` file.\n",
    )),
    Scenario::new("X9 nvm-exec with NODE_VERSION", "nvm-exec (no upstream test)")
        .fixture(&[Node("v20.1.0")])
        .script("NODE_VERSION=20 \"$NVM_EXEC\" node; echo rc=$?")
        .stdout("v20.1.0\nrc=0\n"),
];

#[test]
fn the_contract_holds() {
    check_all(SCENARIOS);
}

#[test]
#[ignore = "needs NVMRC_ORACLE_NVM_SH: run `mise run compat:capture`"]
fn capture_from_nvm_sh() {
    capture_all(SCENARIOS);
}
