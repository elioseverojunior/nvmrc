//! The newest `npm` that works on a `node`: `nvm_install_latest_npm`, as data.

/// What a step installs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Install {
    /// Nothing: the step only says why there is no more to do.
    Nothing,
    /// `npm install -g npm`: the latest.
    Latest,
    /// `npm install -g <argument>`, such as `npm@6`.
    Spec(&'static str),
}

/// One step of an upgrade: what `nvm.sh` says about it, and what it installs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub note: &'static str,
    pub install: Install,
}

type Triple = (u64, u64, u64);

const fn step(note: &'static str, install: &'static str) -> Step {
    Step {
        note,
        install: Install::Spec(install),
    }
}

/// The steps to run, in order, to bring `npm` at `npm` to the newest one that
/// works on `node`.
#[must_use]
pub fn steps(node: Triple, npm: Triple) -> Vec<Step> {
    let is_0_6 = ((0, 6, 0)..(0, 7, 0)).contains(&node);
    let is_0_9 = ((0, 9, 0)..(0, 10, 0)).contains(&node);
    let mut plan = Vec::new();
    if is_0_6 {
        plan.push(step(
            "* `node` v0.6.x can only upgrade to `npm` v1.3.x",
            "npm@1.3",
        ));
    } else if !is_0_9 {
        plan.extend(first_jump(npm));
    }
    if is_0_6 || is_0_9 {
        plan.push(Step {
            note: "* node v0.6 and v0.9 are unable to upgrade further",
            install: Install::Nothing,
        });
    } else if node < (1, 1, 0) {
        plan.push(step(
            "* `npm` v4.5.x is the last version that works on `node` versions < v1.1.0",
            "npm@4.5",
        ));
    } else if node < (4, 0, 0) {
        plan.push(step(
            "* `npm` v5 and higher do not work on `node` versions below v4.0.0",
            "npm@4",
        ));
    } else {
        plan.extend(modern(node, npm));
    }
    plan
}

/// `npm` 1.x and 2.x have to hop through their last release first.
fn first_jump(npm: Triple) -> Option<Step> {
    if ((1, 0, 0)..(2, 0, 0)).contains(&npm) {
        Some(step(
            "* `npm` v1.x needs to first jump to `npm` v1.4.28 to be able to upgrade further",
            "npm@1.4.28",
        ))
    } else if ((2, 0, 0)..(3, 0, 0)).contains(&npm) {
        Some(step(
            "* `npm` v2.x needs to first jump to the latest v2 to be able to upgrade further",
            "npm@2",
        ))
    } else {
        None
    }
}

/// One row of the table for `node` 4 and later: the first row whose
/// condition holds on the `node` gives the `npm` to install.
struct Rule {
    applies: fn(Triple) -> bool,
    note: &'static str,
    install: &'static str,
}

const fn at_least(node: Triple, major: u64, minor: u64) -> bool {
    node.0 > major || (node.0 == major && node.1 >= minor)
}

/// The rules, in the order `nvm.sh` tries them. Each condition is the
/// combination of `node` release lines it names, which is how `nvm.sh` writes
/// them too (`IS_13_OR_ABOVE && !IS_14_LTS_OR_ABOVE`).
const RULES: [Rule; 9] = [
    Rule {
        applies: |n| !at_least(n, 4, 5) || (at_least(n, 5, 0) && !at_least(n, 5, 10)),
        note: "* `npm` `v5.3.x` is the last version that works on `node` 4.x versions below v4.4, or 5.x versions below v5.10, due to `Buffer.alloc`",
        install: "npm@5.3",
    },
    Rule {
        applies: |n| !at_least(n, 4, 7),
        note: "* `npm` `v5.4.1` is the last version that works on `node` `v4.5` and `v4.6`",
        install: "npm@5.4.1",
    },
    Rule {
        applies: |n| !at_least(n, 6, 0),
        note: "* `npm` `v5.x` is the last version that works on `node` below `v6.0.0`",
        install: "npm@5",
    },
    Rule {
        applies: |n| {
            (at_least(n, 6, 0) && !at_least(n, 6, 2)) || (at_least(n, 9, 0) && !at_least(n, 9, 3))
        },
        note: "* `npm` `v6.9` is the last version that works on `node` `v6.0.x`, `v6.1.x`, `v9.0.x`, `v9.1.x`, or `v9.2.x`",
        install: "npm@6.9",
    },
    Rule {
        applies: |n| !at_least(n, 10, 0),
        note: "* `npm` `v6.x` is the last version that works on `node` below `v10.0.0`",
        install: "npm@6",
    },
    Rule {
        applies: |n| {
            !at_least(n, 12, 13)
                || (at_least(n, 13, 0) && !at_least(n, 14, 15))
                || (at_least(n, 15, 0) && !at_least(n, 16, 0))
        },
        note: "* `npm` `v7.x` is the last version that works on `node` `v13`, `v15`, below `v12.13`, or `v14.0` - `v14.15`",
        install: "npm@7",
    },
    Rule {
        applies: |n| {
            (at_least(n, 12, 13) && !at_least(n, 13, 0))
                || (at_least(n, 14, 15) && !at_least(n, 14, 17))
                || (at_least(n, 16, 0) && !at_least(n, 16, 13))
                || (at_least(n, 17, 0) && !at_least(n, 18, 0))
        },
        note: "* `npm` `v8.6` is the last version that works on `node` `v12`, `v14.13` - `v14.16`, or `v16.0` - `v16.12`",
        install: "npm@8.6",
    },
    Rule {
        applies: |n| !at_least(n, 18, 17) || (at_least(n, 19, 0) && !at_least(n, 20, 5)),
        note: "* `npm` `v9.x` is the last version that works on `node` `< v18.17`, `v19`, or `v20.0` - `v20.4`",
        install: "npm@9",
    },
    Rule {
        applies: |n| !at_least(n, 20, 17) || (at_least(n, 21, 0) && !at_least(n, 22, 9)),
        note: "* `npm` `v10.x` is the last version that works on `node` `< v20.17`, `v21`, or `v22.0` - `v22.8`",
        install: "npm@10",
    },
];

const LATEST: Step = Step {
    note: "* Installing latest `npm`; if this does not work on your node version, please report a bug!",
    install: Install::Latest,
};

/// `node` 4 and later: the newest `npm` is bounded by the `node` release.
fn modern(node: Triple, npm: Triple) -> Vec<Step> {
    let Some(rule) = RULES.iter().find(|rule| (rule.applies)(node)) else {
        return vec![LATEST];
    };
    let mut plan = Vec::new();
    // Below `node` 10, `npm` 6 needs `npm` 4.4.4 or later to install it.
    if rule.install == "npm@6" && npm < (4, 4, 4) {
        plan.push(step(
            "* `npm` `v4.4.4` or later is required to install npm v6.14.18",
            "npm@4",
        ));
    }
    plan.push(step(rule.note, rule.install));
    plan
}

#[cfg(test)]
mod tests;
