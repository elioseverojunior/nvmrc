//! `nvm ls`, `nvm which` and `nvm version` (`test/fast/Listing versions`,
//! `test/fast/Listing paths`).

use super::Fixture::{Alias, File, Iojs, Legacy, Node, SystemNode};
use super::{Scenario, capture_all, check_all};

const SCENARIOS: &[Scenario] = &[
    Scenario::new(
        "L1 which <exact version>",
        "fast/Listing paths/Running 'nvm which 0.0.2' should display only version 0.0.2",
    )
    .fixture(&[Legacy("v0.0.2"), Legacy("v0.0.20"), Node("v0.12.0")])
    .script("nvm which 0.0.2; nvm which 0.0.20; nvm which 0.12.0")
    .stdout(concat!(
        "<W>/nvm/v0.0.2/bin/node\n",
        "<W>/nvm/v0.0.20/bin/node\n",
        "<W>/nvm/versions/node/v0.12.0/bin/node\n",
    )),
    Scenario::new(
        "L2 which of a missing version",
        "fast/Listing paths/Running 'nvm which foo' should return a nonzero exit code when not found",
    )
    .script("nvm which nonexistent_version; echo rc=$?")
    .stdout("rc=1\n")
    .stderr(concat!(
        "N/A: version \"nonexistent_version\" is not yet installed.\n",
        "\n",
        "You need to run `nvm install nonexistent_version` to install and use it.\n",
    )),
    Scenario::new(
        "L3 which of an alias to system",
        "fast/Listing paths/Running 'nvm which' should respect alias pointing to system",
    )
    .fixture(&[Alias("default", "system"), SystemNode("v0.0.0")])
    .script(concat!(
        "PATH=\"$D/../sys:$PATH\"; nvm which default; echo rc=$?; ",
        "nvm which system; echo rc=$?",
    ))
    .stdout("<W>/nvm/../sys/node\nrc=0\n<W>/nvm/../sys/node\nrc=0\n"),
    Scenario::new(
        "L4 --no-alias with a pattern",
        "fast/Listing versions/Running 'nvm ls --no-alias' with a pattern errors",
    )
    .script("nvm ls --no-colors --no-alias pattern; echo rc=$?")
    .stdout("rc=55\n")
    .stderr("`--no-alias` is not supported when a pattern is provided.\n"),
    Scenario::new(
        "L5 ls <exact version>",
        "fast/Listing versions/Running 'nvm ls 0.0.2' should display only version 0.0.2",
    )
    .fixture(&[Legacy("v0.0.2"), Legacy("v0.0.20")])
    .script("nvm ls 0.0.2")
    .stdout("         v0.0.2 *\n"),
    Scenario::new(
        "L6 partial patterns and a trailing dot",
        "fast/Listing versions/Running 'nvm ls' with node-like versioning vx.x.x should only list a matched version",
    )
    .fixture(&[Legacy("v0.1.3"), Legacy("v0.2.3"), Legacy("v0.20.3")])
    .script("nvm ls 0.1; nvm ls 0.2; nvm ls v0.2.; nvm ls v0.1.1; echo rc=$?")
    .stdout(concat!(
        "         v0.1.3 *\n",
        "         v0.2.3 *\n",
        "         v0.2.3 *\n",
        "            N/A\n",
        "rc=3\n",
    )),
    Scenario::new(
        "L7 ls of what does not exist",
        "fast/Listing versions/Running 'nvm ls foo', 'nvm ls io', 'nvm ls node_'",
    )
    .fixture(&[Node("v20.1.0")])
    .script(concat!(
        "nvm ls nonexistent_version; echo rc=$?; nvm ls io; echo rc=$?; ",
        "nvm ls node_; echo rc=$?",
    ))
    .stdout(concat!(
        "            N/A\nrc=3\n",
        "            N/A\nrc=3\n",
        "            N/A\nrc=3\n",
    )),
    Scenario::new(
        "L8 ls stable and unstable",
        "fast/Listing versions/Running 'nvm ls stable' and 'nvm ls unstable' should return the appropriate implicit alias",
    )
    .fixture(&[Node("v0.2.3"), Node("v0.3.3"), Node("v0.1.4")])
    .script("nvm ls stable; nvm ls unstable; nvm alias stable 0.1; nvm ls stable")
    .stdout(concat!(
        "         v0.2.3 *\n",
        "         v0.3.3 *\n",
        "stable -> 0.1 (-> v0.1.4 *)\n",
        "         v0.1.4 *\n",
    )),
    Scenario::new(
        "L9 ls default and system with a system node",
        "fast/Listing versions/Running 'nvm ls default' should show system version when available",
    )
    .fixture(&[Alias("default", "system"), Node("v20.1.0"), SystemNode("v0.0.0")])
    .script(concat!(
        "PATH=\"$SYS:$PATH\"; nvm ls default; echo rc=$?; nvm ls system; echo rc=$?; ",
        "nvm ls --no-colors",
    ))
    .stdout(concat!(
        "->       system * (-> v0.0.0)\n",
        "rc=0\n",
        "->       system * (-> v0.0.0)\n",
        "rc=0\n",
        "        v20.1.0 *\n",
        "->       system * (-> v0.0.0)\n",
        "default -> system *\n",
        "iojs -> N/A (default)\n",
        "node -> stable (-> v20.1.0 *) (default)\n",
        "stable -> 20.1 (-> v20.1.0 *) (default)\n",
        "unstable -> N/A (default)\n",
    )),
    Scenario::new(
        "L10 ls system without a system node",
        "fast/Listing versions/Running 'nvm ls system' should include 'system' when appropriate",
    )
    .fixture(&[Node("v20.1.0")])
    .script("nvm ls system; echo rc=$?")
    .stdout("            N/A\nrc=3\n"),
    Scenario::new(
        "L11 ls lists node and io.js by version",
        "fast/Listing versions/Running 'nvm ls' should display all installed versions",
    )
    .fixture(&[Node("v0.12.87"), Node("v0.12.9"), Iojs("v0.1.2"), Iojs("v0.10.2")])
    .script("nvm ls --no-colors --no-alias")
    .stdout(concat!(
        "    iojs-v0.1.2 *\n",
        "   iojs-v0.10.2 *\n",
        "        v0.12.9 *\n",
        "       v0.12.87 *\n",
    )),
    Scenario::new(
        "L12 ls hides dot directories, 'versions' and slashes",
        "fast/Listing versions/Running 'nvm ls' should filter out '.nvm' and 'versions', and not show a trailing slash",
    )
    .fixture(&[
        Legacy("v0.0.1"),
        File("nvm/.nvm/.keep", ""),
        File("nvm/versions/node/.keep", ""),
        File("nvm/v0.0.3/.keep", ""),
    ])
    // stderr silenced: nvm.sh runs `find` on the empty `versions/node/*` glob and
    // prints its complaint, which the upstream test (stdout only) never sees.
    .script("nvm ls --no-colors 2>/dev/null | grep -e '^ *\\.' -e versions -e '/$'; echo rc=$?")
    .stdout("rc=1\n"),
    Scenario::new(
        "L13 ls under set -u",
        "fast/Listing versions/Running 'nvm ls' with nounset should not fail",
    )
    .fixture(&[Node("v0.12.34")])
    .script("set -u; nvm ls 99; echo rc=$?; nvm ls 0.12.00; echo rc=$?")
    .stdout("            N/A\nrc=3\n            N/A\nrc=3\n"),
    Scenario::new(
        "L14 ls with an empty IFS",
        "fast/Listing versions/Using a nonstandard IFS should not break",
    )
    .fixture(&[Node("v0.0.1"), Iojs("v0.1.2")])
    .script("IFS=\"\" nvm ls")
    .stdout(concat!(
        "         v0.0.1 *\n",
        "    iojs-v0.1.2 *\n",
        "iojs -> iojs-v0.1 (-> iojs-v0.1.2 *) (default)\n",
        "node -> stable (-> v0.0.1 *) (default)\n",
        "stable -> 0.0 (-> v0.0.1 *) (default)\n",
        "unstable -> N/A (default)\n",
    )),
    Scenario::new("L15 ls current", "fast/Listing versions/Running 'nvm ls' calls into nvm_alias")
        .fixture(&[Node("v20.1.0"), Node("v18.0.0")])
        .script("nvm use 18 >/dev/null; nvm ls current; nvm ls --no-colors --no-alias")
        .stdout("->      v18.0.0 *\n->      v18.0.0 *\n        v20.1.0 *\n"),
    Scenario::new("L16 version", "nvm version (no upstream test)")
        .fixture(&[Node("v20.1.0")])
        .script("nvm version 20; nvm version node; nvm version foo; echo rc=$?")
        .stdout("v20.1.0\nv20.1.0\nN/A\nrc=3\n"),
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
