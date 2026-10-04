//! `nvm use`, `nvm deactivate` and `nvm current` (the top-level `use` tests).

use super::Fixture::{Alias, Iojs, Legacy, Node, SystemNode};
use super::{Scenario, capture_all, check_all};

const SCENARIOS: &[Scenario] = &[
    Scenario::new(
        "U1 current after deactivate",
        "fast/Running 'nvm current' should display current nvm environment",
    )
    .script("nvm deactivate; echo rc=$?; nvm current")
    .stdout("rc=0\nnone\n")
    .stderr("Could not find <W>/nvm/*/bin in ${PATH}\n"),
    Scenario::new(
        "U2 deactivate unsets what use set",
        "fast/Running 'nvm deactivate' should unset the nvm environment variables",
    )
    .fixture(&[Legacy("v0.2.3")])
    .script(concat!(
        "nvm use --delete-prefix v0.2.3; echo rc=$?; nvm deactivate; echo rc=$?; ",
        "echo \"bin=[${NVM_BIN-}]\"",
    ))
    .stdout(concat!(
        "Now using node v0.2.3\n",
        "rc=0\n",
        "<W>/nvm/*/bin removed from ${PATH}\n",
        "<W>/nvm/*/share/man removed from ${MANPATH}\n",
        "rc=0\n",
        "bin=[]\n",
    )),
    Scenario::new(
        "U3 use and deactivate with hashing disabled",
        "fast/Running 'nvm use' and 'nvm deactivate' with hashing disabled",
    )
    .fixture(&[Node("v20.1.0")])
    .script(concat!(
        "set +h; nvm use --delete-prefix 20; echo rc=$?; nvm deactivate; echo rc=$?; ",
        "echo \"bin=[${NVM_BIN-}]\"",
    ))
    .stdout(concat!(
        "Now using node v20.1.0\n",
        "rc=0\n",
        "<W>/nvm/*/bin removed from ${PATH}\n",
        "<W>/nvm/*/share/man removed from ${MANPATH}\n",
        "rc=0\n",
        "bin=[]\n",
    )),
    Scenario::new(
        "U4 no stale hashed node",
        "fast/Running 'nvm use' does not leave a stale hashed node",
    )
    .fixture(&[Node("v20.1.0"), Node("v18.0.0")])
    .script(concat!(
        "nvm use 20 >/dev/null; node; nvm use 18 >/dev/null; node; ",
        "nvm deactivate >/dev/null; command -v node || echo none",
    ))
    .stdout("v20.1.0\nv18.0.0\nnone\n"),
    Scenario::new(
        "U5 the current symlink",
        "fast/Running 'nvm use x' should create and change the 'current' symlink",
    )
    .fixture(&[Legacy("v0.10.29"), Legacy("v0.11.13")])
    .script(concat!(
        "export NVM_SYMLINK_CURRENT=true; nvm use 0.10.29 >/dev/null; readlink \"$D/current\"; ",
        "nvm use 0.11.13 >/dev/null; readlink \"$D/current\"; ",
        "export NVM_SYMLINK_CURRENT=1; rm \"$D/current\"; nvm use 0.10.29 >/dev/null; ",
        "test -L \"$D/current\" && echo link || echo nolink",
    ))
    .stdout("<W>/nvm/v0.10.29\n<W>/nvm/v0.11.13\nnolink\n"),
    Scenario::new(
        "U6 MANPATH keeps the default",
        "fast/Running 'nvm use' should not clobber the default MANPATH",
    )
    .fixture(&[Legacy("v0.10.2"), Legacy("v0.10.3")])
    .script(concat!(
        "unset MANPATH; nvm use v0.10.2 --silent; echo \"[$MANPATH]\"; ",
        "nvm deactivate --silent; echo \"[${MANPATH-unset}]\"; ",
        "MANPATH=/opt/foo/man; nvm use v0.10.2 --silent; echo \"[$MANPATH]\"; ",
        "nvm use v0.10.3 --silent; echo \"[$MANPATH]\"; ",
        "nvm deactivate --silent; echo \"[$MANPATH]\"",
    ))
    .stdout(concat!(
        "[<W>/nvm/v0.10.2/share/man:]\n",
        "[unset]\n",
        "[<W>/nvm/v0.10.2/share/man:/opt/foo/man:]\n",
        "[<W>/nvm/v0.10.3/share/man:/opt/foo/man:]\n",
        "[/opt/foo/man:]\n",
    )),
    Scenario::new(
        "U7 use iojs",
        "fast/Running 'nvm use iojs' uses latest io.js version",
    )
    .fixture(&[Iojs("v3.99.0")])
    .script("nvm use iojs; echo rc=$?; nvm current")
    .stdout("Now using io.js v3.99.0\nrc=0\niojs-v3.99.0\n"),
    Scenario::new(
        "U8 use system without a system node",
        "fast/Running 'nvm use system' should work as expected",
    )
    .script("nvm use system; echo rc=$?; nvm use --silent system; echo rc=$?")
    .stdout("rc=127\nrc=127\n")
    .stderr("System version of node not found.\n"),
    Scenario::new(
        "U9 use of an alias to system",
        "fast/Running 'nvm use' should respect alias pointing to system",
    )
    .fixture(&[Alias("default", "system"), SystemNode("v0.0.0")])
    .script("PATH=\"$D/../sys:$PATH\"; nvm use default; echo rc=$?")
    .stdout("Now using system version of node: v0.0.0\nrc=0\n"),
    Scenario::new(
        "U10 a circular alias",
        "fast/Running 'nvm use foo' where 'foo' is circular aborts",
    )
    .fixture(&[Alias("foo", "foo"), Alias("circ", "circ")])
    .script(concat!(
        "nvm use foo; echo rc=$?; nvm use --silent foo; echo rc=$?; ",
        "nvm which circ; echo rc=$?",
    ))
    .stdout("rc=8\nrc=8\nrc=8\n")
    .stderr(concat!(
        "The alias \"foo\" leads to an infinite loop. Aborting.\n",
        "The alias \"circ\" leads to an infinite loop. Aborting.\n",
    )),
    Scenario::new(
        "U11 use of a version not installed",
        "nvm_ensure_version_installed",
    )
    .script("nvm use 0.10.29; echo rc=$?; nvm use v18; echo rc=$?")
    .stdout("rc=3\nrc=3\n")
    .stderr(concat!(
        "N/A: version \"v0.10.29\" is not yet installed.\n",
        "\n",
        "You need to run `nvm install 0.10.29` to install and use it.\n",
        "N/A: version \"v18\" is not yet installed.\n",
        "\n",
        "You need to run `nvm install v18` to install and use it.\n",
    )),
    Scenario::new(
        "U12 current of the version in use",
        "nvm_ls_current (no upstream test)",
    )
    .fixture(&[Node("v20.1.0")])
    .script("nvm use 20 >/dev/null; nvm current; nvm ls current")
    .stdout("v20.1.0\n->      v20.1.0 *\n"),
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
