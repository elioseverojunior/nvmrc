//! `nvm alias` and `nvm unalias` (`test/fast/Aliases` and the top-level
//! alias tests).

use super::Fixture::{Alias, File, Iojs, Legacy, Node};
use super::{Scenario, capture_all, check_all};

const SCENARIOS: &[Scenario] = &[
    Scenario::new(
        "A1 alias creates a file",
        "fast/Running 'nvm alias' should create a file in the alias directory",
    )
    .fixture(&[Node("v20.1.0")])
    .script("nvm alias test v20.1.0; echo rc=$?; cat \"$D/alias/test\"")
    .stdout("test -> v20.1.0 *\nrc=0\nv20.1.0\n"),
    Scenario::new(
        "A2 unalias removes the file",
        "fast/Running 'nvm unalias' should remove the alias file",
    )
    .fixture(&[Alias("test", "v0.1.2")])
    .script("nvm unalias test; echo rc=$?; test -e \"$D/alias/test\" && echo still || echo gone")
    .stdout(concat!(
        "Deleted alias test - restore it with `nvm alias \"test\" \"v0.1.2\"`\n",
        "rc=0\ngone\n",
    )),
    Scenario::new(
        "A3 no hash in a name",
        "fast/Aliases/'nvm alias' should not accept aliases with a hash",
    )
    .script("nvm alias foo#bar baz")
    .stderr("Aliases with a comment delimiter (#) are not supported.\n")
    .status(1),
    Scenario::new(
        "A4 no slash in a name",
        "fast/Aliases/'nvm alias' should not accept aliases with slashes",
    )
    .script("nvm alias foo/bar baz")
    .stderr("Aliases in subdirectories are not supported.\n")
    .status(1),
    Scenario::new(
        "A5 unalias: no slash in a name",
        "fast/Aliases/'nvm unalias' should not accept aliases with slashes",
    )
    .script("nvm unalias foo/bar")
    .stderr("Aliases in subdirectories are not supported.\n")
    .status(1),
    Scenario::new(
        "A6 unalias of a built-in alias",
        "fast/Aliases/'nvm unalias' should not accept aliases with names equal to built-in alias",
    )
    .script("for a in node stable unstable iojs system; do nvm unalias $a; echo rc=$?; done")
    .stdout("rc=1\nrc=1\nrc=1\nrc=1\nrc=1\n")
    .stderr(concat!(
        "node is a default (built-in) alias and cannot be deleted.\n",
        "stable is a default (built-in) alias and cannot be deleted.\n",
        "unstable is a default (built-in) alias and cannot be deleted.\n",
        "iojs is a default (built-in) alias and cannot be deleted.\n",
        "system is a default (built-in) alias and cannot be deleted.\n",
    )),
    Scenario::new(
        "A7 unalias of an alias shadowing a built-in one",
        "fast/Aliases/'nvm unalias' should accept aliases when they shadow a built-in alias",
    )
    .script(concat!(
        "nvm alias node stable; echo rc=$?; nvm unalias node; echo rc=$?; ",
        "nvm unalias node; echo rc=$?",
    ))
    .stdout(concat!(
        "node -> stable (-> N/A)\nrc=0\n",
        "Deleted alias node - restore it with `nvm alias \"node\" \"stable\"`\n",
        "rc=0\nrc=1\n",
    ))
    .stderr(concat!(
        "! WARNING: Version 'stable' does not exist.\n",
        "node is a default (built-in) alias and cannot be deleted.\n",
    )),
    Scenario::new(
        "A8 alias again changes the target",
        "fast/Aliases/Running 'nvm alias ˂aliasname˃ ˂target˃' again should change the target",
    )
    .fixture(&[Legacy("v0.0.1"), Legacy("v0.0.2")])
    .script("nvm alias t 0.0.2; nvm alias t; nvm alias t 0.0.1; nvm alias t")
    .stdout(concat!(
        "t -> 0.0.2 (-> v0.0.2 *)\n",
        "t -> 0.0.2 (-> v0.0.2 *)\n",
        "t -> 0.0.1 (-> v0.0.1 *)\n",
        "t -> 0.0.1 (-> v0.0.1 *)\n",
    )),
    Scenario::new(
        "A9 alias <name> lists the names starting with it",
        "fast/Aliases/Running 'nvm alias ˂aliasname˃' should list but one alias",
    )
    .fixture(&[Node("v20.1.0"), Alias("t-1", "20"), Alias("t-10", "20")])
    .script("nvm alias t-1")
    .stdout("t-1 -> 20 (-> v20.1.0 *)\nt-10 -> 20 (-> v20.1.0 *)\n"),
    Scenario::new(
        "A10 implicit aliases",
        "fast/Aliases/Running 'nvm alias' lists implicit aliases when they do not exist",
    )
    .fixture(&[
        Legacy("v0.0.1"),
        Legacy("v0.1.1"),
        Iojs("v0.2.1"),
        Alias("ts", "0.0.1"),
        Alias("tu", "0.1.1"),
    ])
    .script("nvm alias")
    .stdout(concat!(
        "ts -> 0.0.1 (-> v0.0.1 *)\n",
        "tu -> 0.1.1 (-> v0.1.1 *)\n",
        "iojs -> iojs-v0.2 (-> iojs-v0.2.1 *) (default)\n",
        "node -> stable (-> v0.0.1 *) (default)\n",
        "stable -> 0.0 (-> v0.0.1 *) (default)\n",
        "unstable -> 0.1 (-> v0.1.1 *) (default)\n",
    )),
    Scenario::new(
        "A11 default and the whole list",
        "fast/Aliases/Running 'nvm alias' should list all aliases",
    )
    .fixture(&[Node("v20.1.0"), Alias("default", "20")])
    .script("nvm alias default; nvm alias --no-colors")
    .stdout(concat!(
        "default -> 20 (-> v20.1.0 *)\n",
        "default -> 20 (-> v20.1.0 *)\n",
        "iojs -> N/A (default)\n",
        "node -> stable (-> v20.1.0 *) (default)\n",
        "stable -> 20.1 (-> v20.1.0 *) (default)\n",
        "unstable -> N/A (default)\n",
    )),
    Scenario::new(
        "A12 leading blank lines",
        "fast/Aliases/'nvm alias' should ignore leading blank lines in the file",
    )
    .fixture(&[Legacy("v0.0.1"), File("nvm/alias/tb", "\nv0.0.1\n\n")])
    .script("nvm alias tb; nvm which tb")
    .stdout("tb -> v0.0.1 *\n<W>/nvm/v0.0.1/bin/node\n"),
    Scenario::new(
        "A13 uppercase names",
        "fast/Aliases/uppercase alias names should work",
    )
    .fixture(&[Legacy("v0.0.1")])
    .script("nvm alias UPPER_ALIAS v0.0.1; echo rc=$?; cat \"$D/alias/UPPER_ALIAS\"")
    .stdout("UPPER_ALIAS -> v0.0.1 *\nrc=0\nv0.0.1\n"),
    Scenario::new(
        "A14 the lts alias directory",
        "fast/Aliases/lts/'nvm alias' should ensure LTS alias dir exists",
    )
    .script("nvm alias >/dev/null 2>&1; test -d \"$D/alias/lts\" && echo yes || echo no")
    .stdout("yes\n"),
    Scenario::new(
        "A15 an empty target deletes",
        "nvm.sh:5021-5027 (no upstream test)",
    )
    .fixture(&[Node("v20.1.0"), Alias("foo", "20")])
    .script("nvm alias foo \"\"; echo rc=$?; ls \"$D/alias\"")
    .stdout(concat!(
        "Deleted alias foo - restore it with `nvm alias \"foo\" \"20\"`\n",
        "rc=0\nlts\n",
    )),
    Scenario::new(
        "A16 a missing target warns",
        "nvm_make_alias (no upstream test)",
    )
    .script("nvm alias foo 99; echo rc=$?")
    .stdout("foo -> 99 (-> N/A)\nrc=0\n")
    .stderr("! WARNING: Version '99' does not exist.\n"),
    Scenario::new(
        "A17 path traversal",
        "fast/Unit tests/nvm_alias path traversal (adapted)",
    )
    .fixture(&[Node("v20.1.0")])
    .script(concat!(
        "nvm which ../../outside; echo rc=$?; nvm use ../x; echo rc=$?; ",
        "nvm alias ../x 20; echo rc=$?",
    ))
    .stdout("rc=1\nrc=3\nrc=1\n")
    .stderr(concat!(
        "N/A: version \"../../outside\" is not yet installed.\n",
        "\n",
        "You need to run `nvm install ../../outside` to install and use it.\n",
        "N/A: version \"../x\" is not yet installed.\n",
        "\n",
        "You need to run `nvm install ../x` to install and use it.\n",
        "Aliases in subdirectories are not supported.\n",
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
