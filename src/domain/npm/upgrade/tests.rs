use super::*;

fn triple(text: &str) -> Triple {
    let mut parts = text.split('.').map(|part| part.parse().unwrap());
    (
        parts.next().unwrap(),
        parts.next().unwrap(),
        parts.next().unwrap(),
    )
}

/// The `npm install -g` arguments of a plan, `npm` alone for the latest.
fn installs(node: &str, npm: &str) -> Vec<&'static str> {
    steps(triple(node), triple(npm))
        .iter()
        .filter_map(|step| match step.install {
            Install::Nothing => None,
            Install::Latest => Some("npm"),
            Install::Spec(spec) => Some(spec),
        })
        .collect()
}

/// Every expectation below is what `nvm_install_latest_npm` of the real
/// `nvm.sh` printed (with `NVM_DEBUG=1`) for the same node and npm.
const NOTES: [&str; 17] = [
    "* `node` v0.6.x can only upgrade to `npm` v1.3.x",
    "* node v0.6 and v0.9 are unable to upgrade further",
    "* `npm` v4.5.x is the last version that works on `node` versions < v1.1.0",
    "* `npm` v1.x needs to first jump to `npm` v1.4.28 to be able to upgrade further",
    "* `npm` v2.x needs to first jump to the latest v2 to be able to upgrade further",
    "* `npm` v5 and higher do not work on `node` versions below v4.0.0",
    "* `npm` `v5.3.x` is the last version that works on `node` 4.x versions below v4.4, or 5.x versions below v5.10, due to `Buffer.alloc`",
    "* `npm` `v5.4.1` is the last version that works on `node` `v4.5` and `v4.6`",
    "* `npm` `v5.x` is the last version that works on `node` below `v6.0.0`",
    "* `npm` `v6.9` is the last version that works on `node` `v6.0.x`, `v6.1.x`, `v9.0.x`, `v9.1.x`, or `v9.2.x`",
    "* `npm` `v6.x` is the last version that works on `node` below `v10.0.0`",
    "* `npm` `v4.4.4` or later is required to install npm v6.14.18",
    "* `npm` `v7.x` is the last version that works on `node` `v13`, `v15`, below `v12.13`, or `v14.0` - `v14.15`",
    "* `npm` `v8.6` is the last version that works on `node` `v12`, `v14.13` - `v14.16`, or `v16.0` - `v16.12`",
    "* `npm` `v9.x` is the last version that works on `node` `< v18.17`, `v19`, or `v20.0` - `v20.4`",
    "* `npm` `v10.x` is the last version that works on `node` `< v20.17`, `v21`, or `v22.0` - `v22.8`",
    "* Installing latest `npm`; if this does not work on your node version, please report a bug!",
];

/// `(node, npm, expected installs)` with the newest npm that is there.
const CURRENT_NPM: [(&str, &str); 41] = [
    ("0.6.21", "npm@1.3"),
    ("0.9.12", ""),
    ("0.10.48", "npm@4.5"),
    ("0.12.18", "npm@4.5"),
    ("1.0.0", "npm@4.5"),
    ("3.3.1", "npm@4"),
    ("4.0.0", "npm@5.3"),
    ("4.4.7", "npm@5.3"),
    ("4.6.2", "npm@5.4.1"),
    ("4.9.1", "npm@5"),
    ("5.9.1", "npm@5.3"),
    ("5.10.0", "npm@5"),
    ("6.1.0", "npm@6.9"),
    ("6.17.1", "npm@6"),
    ("8.17.0", "npm@6"),
    ("9.2.0", "npm@6.9"),
    ("9.4.0", "npm@6"),
    ("10.24.1", "npm@7"),
    ("12.0.0", "npm@7"),
    ("12.13.0", "npm@8.6"),
    ("13.14.0", "npm@7"),
    ("14.0.0", "npm@7"),
    ("14.15.5", "npm@8.6"),
    ("14.17.0", "npm@9"),
    ("15.14.0", "npm@7"),
    ("16.0.0", "npm@8.6"),
    ("16.12.0", "npm@8.6"),
    ("16.13.0", "npm@9"),
    ("17.9.1", "npm@8.6"),
    ("18.0.0", "npm@9"),
    ("18.16.0", "npm@9"),
    ("18.17.0", "npm@10"),
    ("19.9.0", "npm@9"),
    ("20.4.0", "npm@9"),
    ("20.5.0", "npm@10"),
    ("20.16.0", "npm@10"),
    ("20.17.0", "npm"),
    ("21.7.3", "npm@10"),
    ("22.8.0", "npm@10"),
    ("22.9.0", "npm"),
    ("24.0.0", "npm"),
];

#[test]
fn each_node_gets_the_npm_the_real_script_picks() {
    for (node, expected) in CURRENT_NPM {
        let got = installs(node, "10.2.3").join(",");
        assert_eq!(got, expected, "node {node}");
    }
}

/// `(node, npm, expected installs)` for an npm old enough to need a hop.
const OLD_NPM: [(&str, &str, &str); 35] = [
    ("0.10.48", "1.4.0", "npm@1.4.28,npm@4.5"),
    ("0.10.48", "2.15.0", "npm@2,npm@4.5"),
    ("0.10.48", "3.10.0", "npm@4.5"),
    ("0.10.48", "4.4.3", "npm@4.5"),
    ("0.10.48", "4.4.4", "npm@4.5"),
    ("1.0.0", "1.4.0", "npm@1.4.28,npm@4.5"),
    ("1.0.0", "2.15.0", "npm@2,npm@4.5"),
    ("1.0.0", "3.10.0", "npm@4.5"),
    ("1.0.0", "4.4.3", "npm@4.5"),
    ("1.0.0", "4.4.4", "npm@4.5"),
    ("3.3.1", "1.4.0", "npm@1.4.28,npm@4"),
    ("3.3.1", "2.15.0", "npm@2,npm@4"),
    ("3.3.1", "3.10.0", "npm@4"),
    ("3.3.1", "4.4.3", "npm@4"),
    ("3.3.1", "4.4.4", "npm@4"),
    ("4.9.1", "1.4.0", "npm@1.4.28,npm@5"),
    ("4.9.1", "2.15.0", "npm@2,npm@5"),
    ("4.9.1", "3.10.0", "npm@5"),
    ("4.9.1", "4.4.3", "npm@5"),
    ("4.9.1", "4.4.4", "npm@5"),
    ("8.17.0", "1.4.0", "npm@1.4.28,npm@4,npm@6"),
    ("8.17.0", "2.15.0", "npm@2,npm@4,npm@6"),
    ("8.17.0", "3.10.0", "npm@4,npm@6"),
    ("8.17.0", "4.4.3", "npm@4,npm@6"),
    ("8.17.0", "4.4.4", "npm@6"),
    ("10.24.1", "1.4.0", "npm@1.4.28,npm@7"),
    ("10.24.1", "2.15.0", "npm@2,npm@7"),
    ("10.24.1", "3.10.0", "npm@7"),
    ("10.24.1", "4.4.3", "npm@7"),
    ("10.24.1", "4.4.4", "npm@7"),
    ("20.10.0", "1.4.0", "npm@1.4.28,npm@10"),
    ("20.10.0", "2.15.0", "npm@2,npm@10"),
    ("20.10.0", "3.10.0", "npm@10"),
    ("20.10.0", "4.4.3", "npm@10"),
    ("20.10.0", "4.4.4", "npm@10"),
];

#[test]
fn an_old_npm_may_have_to_hop_through_its_last_release_first() {
    for (node, npm, expected) in OLD_NPM {
        let got = installs(node, npm).join(",");
        assert_eq!(got, expected, "node {node}, npm {npm}");
    }
}

/// `(node, npm, indexes into NOTES)`: what is said, in order.
const SAID: [(&str, &str, &[usize]); 39] = [
    ("0.6.21", "10.2.3", &[0, 1]),
    ("0.9.12", "10.2.3", &[1]),
    ("0.10.48", "10.2.3", &[2]),
    ("0.10.48", "1.4.0", &[3, 2]),
    ("0.10.48", "2.15.0", &[4, 2]),
    ("3.3.1", "10.2.3", &[5]),
    ("3.3.1", "1.4.0", &[3, 5]),
    ("3.3.1", "2.15.0", &[4, 5]),
    ("4.0.0", "10.2.3", &[6]),
    ("4.0.0", "1.4.0", &[3, 6]),
    ("4.0.0", "2.15.0", &[4, 6]),
    ("4.6.2", "10.2.3", &[7]),
    ("4.6.2", "1.4.0", &[3, 7]),
    ("4.6.2", "2.15.0", &[4, 7]),
    ("4.9.1", "10.2.3", &[8]),
    ("4.9.1", "1.4.0", &[3, 8]),
    ("4.9.1", "2.15.0", &[4, 8]),
    ("6.1.0", "10.2.3", &[9]),
    ("6.1.0", "1.4.0", &[3, 9]),
    ("6.1.0", "2.15.0", &[4, 9]),
    ("6.17.1", "10.2.3", &[10]),
    ("6.17.1", "3.10.0", &[11, 10]),
    ("6.17.1", "1.4.0", &[3, 11, 10]),
    ("6.17.1", "2.15.0", &[4, 11, 10]),
    ("10.24.1", "10.2.3", &[12]),
    ("10.24.1", "1.4.0", &[3, 12]),
    ("10.24.1", "2.15.0", &[4, 12]),
    ("12.13.0", "10.2.3", &[13]),
    ("12.13.0", "1.4.0", &[3, 13]),
    ("12.13.0", "2.15.0", &[4, 13]),
    ("14.17.0", "10.2.3", &[14]),
    ("14.17.0", "1.4.0", &[3, 14]),
    ("14.17.0", "2.15.0", &[4, 14]),
    ("18.17.0", "10.2.3", &[15]),
    ("18.17.0", "1.4.0", &[3, 15]),
    ("18.17.0", "2.15.0", &[4, 15]),
    ("20.17.0", "10.2.3", &[16]),
    ("20.17.0", "1.4.0", &[3, 16]),
    ("20.17.0", "2.15.0", &[4, 16]),
];

#[test]
fn every_step_says_what_the_real_script_says() {
    for (node, npm, indexes) in SAID {
        let said: Vec<&str> = steps(triple(node), triple(npm))
            .iter()
            .map(|step| step.note)
            .collect();
        let expected: Vec<&str> = indexes.iter().map(|index| NOTES[*index]).collect();
        assert_eq!(said, expected, "node {node}, npm {npm}");
    }
}

#[test]
fn the_newest_node_installs_plain_npm() {
    assert_eq!(installs("24.0.0", "10.2.3"), ["npm"]);
    let plan = steps(triple("24.0.0"), triple("10.2.3"));
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].install, Install::Latest);
}

#[test]
fn node_0_6_and_0_9_are_told_they_cannot_go_further_without_an_install() {
    let plan = steps(triple("0.9.12"), triple("1.4.0"));
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].install, Install::Nothing);
    assert!(plan[0].note.contains("unable to upgrade further"));
}
