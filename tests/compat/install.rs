//! `install`, `uninstall`, `ls-remote` and `version-remote`, offline or
//! against the local mirror (`common::NODE_INDEX`: v20.10.0 Iron, v18.19.0
//! Hydrogen; no io.js release).

use super::Fixture::{Alias, File, Legacy, Mirror, Node};
use super::{Scenario, capture_all, check_all};

const SCENARIOS: &[Scenario] = &[
    Scenario::new(
        "I1 install of an invalid version",
        "fast/Running 'nvm install' with an invalid version fails nicely",
    )
    .script("nvm install invalid.invalid; echo rc=$?")
    .stdout("rc=3\n")
    .stderr(
        "Version 'invalid.invalid' not found - try `nvm ls-remote` to browse available versions.\n",
    ),
    Scenario::new(
        "I2 install --offline",
        "fast/Unit tests/nvm install --offline",
    )
    .script("nvm install --offline 999.999.999; echo rc=$?")
    .stdout("rc=3\n")
    .stderr(concat!(
        "Version '999.999.999' not found locally or in cache - try `nvm ls` ",
        "to browse available versions.\n",
    )),
    Scenario::new(
        "I3 LTS names",
        "fast/Unit tests/nvm install with nonlowercase LTS name",
    )
    .fixture(&[Mirror])
    .script("nvm install lts/ARGON; echo rc=$?; nvm install --lts 0.12; echo rc=$?")
    .stdout("rc=3\nrc=3\n")
    .stderr(concat!(
        "LTS names must be lowercase\n",
        "Version with LTS filter 'ARGON' not found - try `nvm ls-remote --lts=ARGON` ",
        "to browse available versions.\n",
        "Version '0.12' (with LTS filter) not found - try `nvm ls-remote --lts` ",
        "to browse available versions.\n",
    )),
    Scenario::new("I4 version-remote", "fast/Unit tests/nvm version-remote")
        .fixture(&[Mirror])
        .script(concat!(
            "nvm version-remote 20; nvm version-remote --lts; nvm version-remote lts/hydrogen; ",
            "nvm version-remote --lts=hydrogen 18; nvm version-remote lts/foo; echo rc=$?; ",
            "nvm version-remote 99; echo rc=$?; nvm version-remote --foo bar; echo rc=$?",
        ))
        .stdout(concat!(
            "v20.10.0\n",
            "v20.10.0\n",
            "v18.19.0\n",
            "v18.19.0\n",
            "N/A\n",
            "rc=3\n",
            "N/A\n",
            "rc=3\n",
            "rc=55\n",
        ))
        .stderr("Unsupported option \"--foo\".\n"),
    Scenario::new("I5 ls-remote", "fast/Unit tests/nvm ls-remote")
        .fixture(&[Mirror])
        .script(concat!(
            "nvm ls-remote --no-colors; echo rc=$?; nvm ls-remote --lts --no-colors; ",
            "echo rc=$?; nvm ls-remote 99; echo rc=$?",
        ))
        .stdout(concat!(
            "       v18.19.0   (Latest LTS: Hydrogen)\n",
            "       v20.10.0   (Latest LTS: Iron)\n",
            "rc=3\n",
            "       v18.19.0   (Latest LTS: Hydrogen)\n",
            "       v20.10.0   (Latest LTS: Iron)\n",
            "rc=0\n",
            "            N/A\n",
            "rc=3\n",
        ))
        .deviation("D9"),
    Scenario::new(
        "I6 ls-remote under set -u",
        "fast/Unit tests/nvm ls-remote with nounset should not fail",
    )
    .fixture(&[Mirror])
    .script("set -u; nvm ls-remote --lts --no-colors >/dev/null; echo rc=$?")
    .stdout("rc=0\n"),
    Scenario::new(
        "I7 uninstall removes the aliases of the version",
        "fast/Running 'nvm uninstall' should clean up aliases pointing to uninstalled version",
    )
    .fixture(&[Node("v20.1.0"), Node("v18.0.0"), Alias("ta", "v20.1.0")])
    .script("nvm uninstall v20.1.0; echo rc=$?; ls \"$D/alias\"; ls \"$D/versions/node\"")
    .stdout(concat!(
        "Uninstalled node v20.1.0\n",
        "Deleted alias ta - restore it with `nvm alias \"ta\" \"v20.1.0\"`\n",
        "rc=0\n",
        "v18.0.0\n",
    )),
    Scenario::new(
        "I8 uninstall removes the directory",
        "fast/Running 'nvm uninstall' should remove the appropriate directory",
    )
    .fixture(&[Legacy("v0.0.1")])
    .script("nvm uninstall v0.0.1; echo rc=$?; test -d \"$D/v0.0.1\" && echo still || echo gone")
    .stdout("Uninstalled node v0.0.1\nrc=0\ngone\n"),
    Scenario::new(
        "I9 uninstall of an inferred version",
        "fast/Running 'nvm uninstall' with an inferred version shows the inferred message",
    )
    .fixture(&[File("nvm/v0.0.1/.keep", "")])
    .script("nvm uninstall 0.0; echo rc=$?")
    .stdout("rc=0\n")
    .stderr("Version 'v0.0.1' (inferred from 0.0) is not installed.\n"),
    Scenario::new(
        "I10 uninstall of a version not installed",
        "fast/Running 'nvm uninstall' with an uninstalled version shows the requested version",
    )
    .script("nvm uninstall 22; echo rc=$?")
    .stdout("rc=0\n")
    .stderr("Version '22' is not installed.\n"),
    Scenario::new(
        "I11 mirror injection characters",
        "spec 8.5; nvm_get_mirror",
    )
    .script(concat!(
        "NVM_NODEJS_ORG_MIRROR=\"http://x/;id\" nvm ls-remote; echo rc=$?; ",
        "NVM_NODEJS_ORG_MIRROR=\"http://x/;id\" nvm install 20; echo rc=$?",
    ))
    .stdout("            N/A\nrc=3\nrc=3\n")
    .stderr(concat!(
        "$NVM_NODEJS_ORG_MIRROR and $NVM_IOJS_ORG_MIRROR may only contain a URL\n",
        "$NVM_NODEJS_ORG_MIRROR and $NVM_IOJS_ORG_MIRROR may only contain a URL\n",
        "Version '20' not found - try `nvm ls-remote` to browse available versions.\n",
    ))
    .deviation("D9"),
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
