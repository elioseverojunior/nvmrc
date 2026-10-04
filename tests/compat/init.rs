//! Loading `nvm` (`test/sourcing`, in every POSIX shell) and the commands
//! nvmrc answers differently (bash only).

use super::Fixture::{Alias, Node};
use super::{POSIX, Scenario, capture_all, check_all};

/// clap's answer to a command it does not know.
macro_rules! unknown {
    ($name:literal) => {
        concat!(
            "error: unrecognized subcommand '",
            $name,
            "'\n\nUsage: nvm <COMMAND>\n\nFor more information, try '--help'.\n"
        )
    };
}

const SCENARIOS: &[Scenario] = &[
    Scenario::new(
        "S1 the default is used",
        "sourcing/Sourcing nvm.sh should use the default if available and no nvm node is loaded",
    )
    .shells(POSIX)
    .fixture(&[Node("v20.1.0"), Alias("default", "20")])
    .script("@load@\nnvm current\nnvm alias default")
    .stdout("v20.1.0\ndefault -> 20 (-> v20.1.0 *)\n"),
    Scenario::new(
        "S2 --no-use",
        "sourcing/Sourcing nvm.sh with --no-use should not use anything",
    )
    .shells(POSIX)
    .fixture(&[Node("v20.1.0"), Alias("default", "20")])
    .script("@load-no-use@\nnvm current")
    .stdout("none\n"),
    Scenario::new(
        "S3 an active version is kept",
        "sourcing/Sourcing nvm.sh should keep version if one is active",
    )
    .shells(POSIX)
    .fixture(&[Node("v18.0.0"), Node("v20.1.0"), Alias("default", "20")])
    .script("@load-no-use@\nnvm use 18 >/dev/null\n@load@\nnvm current")
    .stdout("v18.0.0\n"),
    Scenario::new(
        "S4 no default is not a failure",
        "sourcing/Sourcing nvm.sh with no default should return 0",
    )
    .shells(POSIX)
    .script("set -e\n@load@\necho ok")
    .stdout("ok\n"),
    Scenario::new(
        "S5 the caller's parameters",
        "fast/Sourcing nvm.sh should not modify parameters of caller",
    )
    .shells(POSIX)
    .script("set -- yes\n@load-no-use@\necho \"$1\"")
    .stdout("yes\n"),
    Scenario::new(
        "S6 a bare nvm",
        "fast/Sourcing nvm.sh should make the nvm command available",
    )
    .shells(POSIX)
    .script("set -e\n@load-no-use@\nnvm >/dev/null\necho rc=$?")
    .stdout("rc=0\n"),
    Scenario::new(
        "S7 a trailing slash in NVM_DIR",
        "fast/nvm should remove the last trailing slash in $NVM_DIR",
    )
    .script("export NVM_DIR=\"$D/\"\n@load-no-use@\nnvm cache dir")
    .stdout("<W>/nvm/.cache\n")
    // nvm.sh also warns on stderr that $NVM_DIR has trailing slashes.
    .deviation("D17"),
    Scenario::new(
        "S8 unload",
        "fast/Running 'nvm unload' should unset all function and variables",
    )
    .script("nvm unload; echo rc=$?")
    .stdout("rc=127\n")
    .stderr(unknown!("unload"))
    .deviation("D5"),
    Scenario::new(
        "S9 an unknown command",
        "nvm.sh: help dump on stderr, status 127",
    )
    .script("nvm bogus; echo rc=$?")
    .stdout("rc=127\n")
    .stderr(unknown!("bogus"))
    .deviation("D6"),
    Scenario::new("S10 set-colors", "fast/Unit tests/nvm set_colors")
        .script(concat!(
            "nvm set-colors rgbyc; echo rc=$? $NVM_COLORS; nvm set-colors rgby; echo rc=$?; ",
            "nvm set-colors p3gq7; echo rc=$?",
        ))
        .stdout(concat!(
            "Setting colors to: r g b y c\n",
            "WARNING: Colors may not display because they are not supported in this shell.\n",
            "rc=0 rgbyc\n",
            "\n",
            "rc=0\n",
            "\n",
            "rc=0\n",
        ))
        .stderr(concat!(
            "\u{1b}[1;37mPlease pass in five \u{1b}[1;31mvalid color codes\u{1b}[1;37m. ",
            "Choose from: rRgGbBcCyYmMkKeW\u{1b}[0m\n",
            "\u{1b}[1;37mPlease pass in five \u{1b}[1;31mvalid color codes\u{1b}[1;37m. ",
            "Choose from: rRgGbBcCyYmMkKeW\u{1b}[0m\n",
        ))
        .deviation("D11"),
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
