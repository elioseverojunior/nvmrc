use super::*;

fn version(text: &str) -> Version {
    text.parse().unwrap()
}

fn platform(os: &str, arch: &str) -> Platform {
    Platform::from_host(os, arch, false).unwrap()
}

#[test]
fn rust_names_become_the_names_nodejs_org_uses() {
    assert_eq!(platform("linux", "x86_64").arch, "x64");
    assert_eq!(platform("linux", "aarch64").arch, "arm64");
    assert_eq!(
        platform("macos", "aarch64"),
        Platform {
            os: Os::Darwin,
            arch: "arm64".into()
        }
    );
    assert_eq!(platform("linux", "arm").arch, "armv7l");
    assert_eq!(platform("linux", "loongarch64").arch, "loong64");
    assert_eq!(platform("linux", "s390x").arch, "s390x");
}

#[test]
fn an_unsupported_operating_system_has_no_platform() {
    assert_eq!(Platform::from_host("freebsd", "x86_64", false), None);
    assert_eq!(Platform::from_host("windows", "x86_64", false), None);
}

#[test]
fn alpine_uses_the_musl_builds_of_x64_and_arm64_only() {
    assert_eq!(
        Platform::from_host("linux", "x86_64", true).unwrap().arch,
        "x64-musl"
    );
    assert_eq!(
        Platform::from_host("linux", "aarch64", true).unwrap().arch,
        "arm64-musl"
    );
    assert_eq!(
        Platform::from_host("linux", "arm", true).unwrap().arch,
        "armv7l"
    );
    assert_eq!(
        Platform::from_host("macos", "x86_64", true).unwrap().arch,
        "x64"
    );
}

#[test]
fn the_slug_names_flavor_version_os_and_arch() {
    let linux = platform("linux", "x86_64");
    assert_eq!(
        linux.download_slug(&version("v20.10.0")),
        "node-v20.10.0-linux-x64"
    );
    assert_eq!(
        linux.download_slug(&version("iojs-v3.3.1")),
        "iojs-v3.3.1-linux-x64"
    );
}

#[test]
fn old_arm_builds_are_arm_pi_and_merged_ones_are_not() {
    let pi = platform("linux", "arm");
    assert_eq!(
        pi.download_slug(&version("v0.12.18")),
        "node-v0.12.18-linux-arm-pi"
    );
    assert_eq!(
        pi.download_slug(&version("iojs-v3.3.1")),
        "iojs-v3.3.1-linux-arm-pi"
    );
    assert_eq!(
        pi.download_slug(&version("v4.0.0")),
        "node-v4.0.0-linux-armv7l"
    );
}

#[test]
fn apple_silicon_runs_the_x64_build_before_node_16() {
    let mac = platform("macos", "aarch64");
    assert_eq!(
        mac.download_slug(&version("v14.21.3")),
        "node-v14.21.3-darwin-x64"
    );
    assert_eq!(
        mac.download_slug(&version("v16.0.0")),
        "node-v16.0.0-darwin-arm64"
    );
    let linux = platform("linux", "aarch64");
    assert_eq!(
        linux.download_slug(&version("v14.21.3")),
        "node-v14.21.3-linux-arm64"
    );
}

#[test]
fn binaries_start_at_node_0_8_6() {
    assert!(!binary_available(&version("v0.8.5")));
    assert!(binary_available(&version("v0.8.6")));
    assert!(binary_available(&version("iojs-v1.0.0")));
}
