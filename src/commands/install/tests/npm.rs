use super::*;

const NPM: &str = "/n/versions/node/v20.10.0/bin/npm";

fn with_npm(world: World, script: FakeProcess) -> World {
    World {
        process: script,
        ..world
    }
}

fn npm_calls(world: &World) -> Vec<String> {
    world
        .process
        .executed()
        .iter()
        .map(|i| i.args.join(" "))
        .collect()
}

#[test]
fn a_fresh_install_makes_the_default_alias_before_it_upgrades_npm() {
    let process = FakeProcess::default()
        .with_success(NPM, "--version", "10.2.3\n")
        .with_success(NPM, "install -g npm@10", "added\n");
    let world = with_npm(World::new(), process);
    let output = world.run("--latest-npm 20").unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    let out = lines(&output.stdout);
    assert_eq!(out[0], "Downloading and installing node v20.10.0...");
    assert_eq!(
        out[1],
        "Creating default alias: default -> 20 (-> v20.10.0 *)"
    );
    assert_eq!(
        out[2],
        "Attempting to upgrade to the latest working version of npm..."
    );
    assert_eq!(*out.last().unwrap(), "* npm upgraded to: v10.2.3");
}

#[test]
fn an_installed_version_still_gets_the_npm_steps_and_the_default_alias_after_them() {
    let process = FakeProcess::default()
        .with_success(NPM, "--version", "10.2.3\n")
        .with_success(NPM, "install -g npm@10", "added\n");
    let world = with_npm(World::new(), process);
    world.run("20").unwrap();
    let again = world.run("--latest-npm 20").unwrap();
    assert_eq!(again.stderr, "v20.10.0 is already installed.");
    assert!(
        again
            .stdout
            .starts_with("Attempting to upgrade to the latest working version of npm...")
    );
    assert!(npm_calls(&world).contains(&"install -g npm@10".to_owned()));
}

#[test]
fn a_failing_npm_step_is_the_status_of_the_install_but_the_version_stays() {
    let world = World::new();
    let output = world.run("--latest-npm 20").unwrap();
    assert_eq!(output.status, NvmExitCode::MissingTarget);
    assert!(output.stderr.ends_with("Unable to obtain npm version."));
    assert!(world.installed());
    assert!(world.text("/n/alias/default").is_some());
}

#[test]
fn the_alias_of_an_installed_version_is_not_made_when_an_npm_step_failed() {
    let world = World::new();
    world.run("20").unwrap();
    let output = world.run("--latest-npm --alias=work 20").unwrap();
    assert_eq!(output.status, NvmExitCode::MissingTarget);
    assert!(world.text("/n/alias/work").is_none());
}

#[test]
fn skip_default_packages_does_not_read_the_file() {
    let world = World::new();
    world
        .fs
        .write_file(Path::new("/n/default-packages"), "a b\n")
        .unwrap();
    assert_eq!(
        world.run("--skip-default-packages 20").unwrap().status,
        NvmExitCode::Success
    );
    assert_eq!(World::new().run("20").unwrap().status, NvmExitCode::Success);
}
