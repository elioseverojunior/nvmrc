use super::*;

fn parsed(output: &str) -> (Vec<String>, Vec<String>) {
    let packages = parse(output);
    (packages.installs, packages.links)
}

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|word| (*word).to_owned()).collect()
}

/// Every expectation is what `nvm_npm_global_modules` of the real `nvm.sh`
/// printed for the same `npm list -g --depth=0`.
#[test]
fn plain_and_scoped_packages_are_installed_but_npm_and_corepack_are_not() {
    let output = "/home/me/.nvm/versions/node/v20/lib\n├── corepack@0.20.0\n├── npm@10.2.3\n├── yarn@1.22.19\n└── @angular/cli@17.0.0\n";
    let (installs, links) = parsed(output);
    assert_eq!(installs, words(&["yarn@1.22.19", "@angular/cli@17.0.0"]));
    assert!(links.is_empty());
}

#[test]
fn links_unmet_peers_and_trailing_words_are_handled() {
    let output = "/x/lib\n├── corepack@0.20.0\n├── npm@10.2.3\n├── lodash@4.17.21 extraneous\n├── mylink@1.0.0 -> /home/me/src/mylink\n├── typescript@5.3.3\n└── UNMET PEER DEPENDENCY react@^18\n";
    let (installs, links) = parsed(output);
    assert_eq!(installs, words(&["lodash@4.17.21", "typescript@5.3.3"]));
    assert_eq!(links, words(&["/home/me/src/mylink"]));
}

#[test]
fn an_empty_list_has_nothing() {
    assert_eq!(parsed("/x/lib\n└── (empty)\n"), (vec![], vec![]));
    assert_eq!(parsed("/x/lib\n"), (vec![], vec![]));
    assert_eq!(parsed(""), (vec![], vec![]));
}

#[test]
fn old_npm_tree_drawings_work_too() {
    let (installs, _) = parsed("/x/lib\n+-- foo@1.0.0\n`-- bar@2.0.0\n");
    assert_eq!(installs, words(&["foo@1.0.0", "bar@2.0.0"]));
}

#[test]
fn a_line_that_does_not_fit_stays_as_it_is_and_the_last_at_wins() {
    let (installs, _) = parsed("/x/lib\n├── weird line without at\n├── a@b@c d\n");
    assert_eq!(
        installs,
        words(&["├──", "weird", "line", "without", "at", "a@b@c"])
    );
}
