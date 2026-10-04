//! The exit codes the spec left pending, pinned end to end: 2, 4, 5, 6, 7
//! and 11. (10 is internal to nvm.sh, turned into 11 by `nvm use`; 42 only
//! comes from `nvm debug`, which nvmrc does not have.)

use super::Fixture::{Mirror, Node};
use super::{Scenario, capture_all, check_all};

const SCENARIOS: &[Scenario] = &[
    Scenario::new("C2 reinstall-packages from the current version", "nvm.sh:5144 (no upstream test)")
        .fixture(&[Node("v20.1.0")])
        .script("nvm use 20 >/dev/null; nvm reinstall-packages 20; echo rc=$?")
        .stdout("rc=2\n")
        .stderr("Can not reinstall packages from the current version of node.\n"),
    Scenario::new(
        "C4 C5 C6 --reinstall-packages-from",
        "fast/Running 'nvm install' with '--reinstall-packages-from' requires a valid version",
    )
    .fixture(&[Mirror, Node("v18.19.0")])
    .script(concat!(
        "nvm install v20.10.0 --reinstall-packages-from=0.11; echo rc=$?; ",
        "nvm install v20.10.0 --reinstall-packages-from=20.10.0; echo rc=$?; ",
        "nvm install v20.10.0 --reinstall-packages-from; echo rc=$?; ",
        "nvm install v20.10.0 --reinstall-packages-from=; echo rc=$?",
    ))
    .stdout("rc=5\nrc=4\nrc=6\nrc=6\n")
    .stderr(concat!(
        "If --reinstall-packages-from is provided, it must point to an installed version of node.\n",
        "You can't reinstall global packages from the same version of node you're installing.\n",
        "If --reinstall-packages-from is provided, it must point to an installed version of node using `=`.\n",
        "If --reinstall-packages-from is provided, it must point to an installed version of node.\n",
    )),
    Scenario::new("C6 -s with -b", "fast/Unit tests/nvm install -s and -b conflict")
        .script("nvm install -s -b 20; echo rc=$?; nvm install -b -s 20; echo rc=$?")
        .stdout("rc=6\nrc=6\n")
        .stderr(concat!(
            "-s and -b cannot be set together since they would skip install from both binary and source\n",
            "-s and -b cannot be set together since they would skip install from both binary and source\n",
        )),
    Scenario::new("C7 the version floor", "fast/Unit tests/nvm install NVM_MIN_VERSION")
        .fixture(&[Mirror])
        .script(concat!(
            "NVM_MIN_VERSION=24 nvm install 20; echo rc=$?; ",
            "NVM_MIN_VERSION=bogus nvm install 20; echo rc=$?; ",
            "echo 24 > \"$D/min-version\"; nvm install 18; echo rc=$?",
        ))
        .stdout("rc=7\nrc=7\nrc=7\n")
        .stderr(concat!(
            "Version v20.10.0 is below the minimum allowed version v24.0.0.\n",
            "Lower or unset NVM_MIN_VERSION (or edit <W>/nvm/min-version) to install it.\n",
            "Invalid minimum version 'bogus' (from NVM_MIN_VERSION or <W>/nvm/min-version).\n",
            "Version v18.19.0 is below the minimum allowed version v24.0.0.\n",
            "Lower or unset NVM_MIN_VERSION (or edit <W>/nvm/min-version) to install it.\n",
        )),
    Scenario::new("C11 an npm prefix nvm cannot work with", "nvm.sh:4690 (no upstream test)")
        .fixture(&[Node("v20.1.0")])
        .script(concat!(
            "export PREFIX=/x; nvm use 20; echo rc=$?; unset PREFIX; ",
            "export npm_config_prefix=/y; nvm use 20; echo rc=$?; unset npm_config_prefix; ",
            "echo prefix=/z > \"$HOME/.npmrc\"; nvm use 20; echo rc=$?",
        ))
        .stdout("rc=11\nrc=11\nrc=11\n")
        .stderr(concat!(
            "nvm is not compatible with the \"PREFIX\" environment variable: currently set to \"/x\"\n",
            "Run `unset PREFIX` to unset it.\n",
            "nvm is not compatible with the \"npm_config_prefix\" environment variable: ",
            "currently set to \"/y\"\n",
            "Run `unset npm_config_prefix` to unset it.\n",
            "Your user\u{2019}s .npmrc file (${HOME}/.npmrc)\n",
            "has a `globalconfig` and/or a `prefix` setting, which are incompatible with nvm.\n",
            "Run `nvm use --delete-prefix v20.1.0` to unset it.\n",
        )),
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
