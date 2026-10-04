//! `.nvmrc` handling by `use`, `which` and `install`.

use super::Fixture::{File, Legacy, Node, SystemNode};
use super::{Scenario, capture_all, check_all};

const SCENARIOS: &[Scenario] = &[
    Scenario::new("N1 a CR is dropped", "fast/Running 'nvm use' should drop CR char automatically")
        .fixture(&[Node("v20.1.0"), File("proj/.nvmrc", "20.1.0\r\n")])
        .script("nvm which; echo rc=$?; nvm use; echo rc=$?")
        .stdout(concat!(
            "Found '<W>/proj/.nvmrc' with version <20.1.0>\n",
            "<W>/nvm/versions/node/v20.1.0/bin/node\n",
            "rc=0\n",
            "Found '<W>/proj/.nvmrc' with version <20.1.0>\n",
            "Now using node v20.1.0\n",
            "rc=0\n",
        )),
    Scenario::new("N2 no version and no .nvmrc", "nvm use without a version (no upstream test)")
        .script("nvm use; echo rc=$?")
        .stdout("rc=127\n")
        .stderr(concat!(
            "No version provided and no .nvmrc file found\n",
            "Please see `nvm --help` or https://github.com/nvm-sh/nvm#nvmrc for more information.\n",
        )),
    Scenario::new("N3 an empty .nvmrc", "nvm_process_nvmrc (no upstream test)")
        .fixture(&[File("proj/.nvmrc", "\n")])
        .script("nvm use; echo rc=$?; nvm which; echo rc=$?; nvm install; echo rc=$?")
        .stdout("rc=127\nrc=127\nrc=127\n")
        .stderr(concat!(
            "invalid .nvmrc!\n",
            "all non-commented content (anything after # is a comment) must be either:\n",
            "  - a single bare nvm-recognized version-ish\n",
            "  - or, multiple distinct key-value pairs, each key/value separated by a single equals sign (=)\n",
            "\n",
            "additionally, a single bare nvm-recognized version-ish must be present (after stripping comments).\n",
            "\n",
            "non-commented content parsed:\n",
            "Please see `nvm --help` or https://github.com/nvm-sh/nvm#nvmrc for more information.\n",
            "invalid .nvmrc!\n",
            "all non-commented content (anything after # is a comment) must be either:\n",
            "  - a single bare nvm-recognized version-ish\n",
            "  - or, multiple distinct key-value pairs, each key/value separated by a single equals sign (=)\n",
            "\n",
            "additionally, a single bare nvm-recognized version-ish must be present (after stripping comments).\n",
            "\n",
            "non-commented content parsed:\n",
            "Usage: nvm which [current | <version>]\n",
            "  Provide a <version>, or run from a directory containing an .nvmrc file.\n",
            "  Run `nvm --help` for full help.\n",
            "invalid .nvmrc!\n",
            "all non-commented content (anything after # is a comment) must be either:\n",
            "  - a single bare nvm-recognized version-ish\n",
            "  - or, multiple distinct key-value pairs, each key/value separated by a single equals sign (=)\n",
            "\n",
            "additionally, a single bare nvm-recognized version-ish must be present (after stripping comments).\n",
            "\n",
            "non-commented content parsed:\n",
            "Usage: nvm install [<version>]\n",
            "  Provide a <version>, or run from a directory containing an .nvmrc file.\n",
            "  Run `nvm --help` for full help.\n",
        )),
    Scenario::new("N4 system in .nvmrc", "fast/Running 'nvm use' should respect system in .nvmrc")
        .fixture(&[File("proj/.nvmrc", "system\n"), SystemNode("v0.0.0")])
        .script("PATH=\"$D/../sys:$PATH\"; nvm use; echo rc=$?")
        .stdout(concat!(
            "Found '<W>/proj/.nvmrc' with version <system>\n",
            "Now using system version of node: v0.0.0\n",
            "rc=0\n",
        )),
    Scenario::new("N5 use --save", "fast/Unit tests/Running 'nvm use --save' works as expected'")
        .fixture(&[Legacy("v0.2.4")])
        .script(concat!(
            "nvm use --save v0.2.4 >/dev/null; cat .nvmrc; rm .nvmrc; ",
            "nvm use -w --silent v0.2.4; echo \"rc=$?\"; cat .nvmrc",
        ))
        .stdout("v0.2.4\nrc=0\nv0.2.4\n"),
    Scenario::new("N6 which of an .nvmrc version not installed", "nvm which (no upstream test)")
        .fixture(&[File("proj/.nvmrc", "99\n")])
        .script("nvm which; echo rc=$?")
        .stdout("Found '<W>/proj/.nvmrc' with version <99>\nrc=1\n")
        .stderr(concat!(
            "N/A: version \"v99\" is not yet installed.\n",
            "\n",
            "You need to run `nvm install 99` to install and use it.\n",
        )),
    Scenario::new("N7 both streams read together", "fast/Running 'nvm-exec' should display required node version")
        .fixture(&[File("proj/.nvmrc", "99\n")])
        .script("nvm use 2>&1; echo rc=$?")
        .stdout(concat!(
            "N/A: version \"v99\" is not yet installed.\n",
            "\n",
            "You need to run `nvm install` to install and use the node version specified in `.nvmrc`.\n",
            "Found '<W>/proj/.nvmrc' with version <99>\n",
            "rc=3\n",
        ))
        .deviation("D1"),
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
