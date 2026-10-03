use super::*;
use crate::domain::version::Version;

fn xz(version: &str, os: Os) -> bool {
    let version = version.parse::<Version>().unwrap();
    Compression::preferred(&version, os) == Compression::Xz
}

#[test]
fn it_prefers_xz_from_node_4() {
    assert!(xz("v4.0.0", Os::Linux));
    assert!(xz("v20.10.0", Os::Darwin));
    assert!(xz("v22.0.0", Os::Aix));
}

#[test]
fn it_prefers_xz_from_the_last_patches_of_0_10_and_0_12() {
    assert!(!xz("v0.10.41", Os::Linux));
    assert!(xz("v0.10.42", Os::Linux));
    assert!(xz("v0.10.48", Os::Linux));
    assert!(!xz("v0.12.9", Os::Linux));
    assert!(xz("v0.12.10", Os::Linux));
    assert!(xz("v0.12.18", Os::Linux));
}

#[test]
fn it_uses_gzip_for_the_versions_between() {
    assert!(!xz("v0.8.6", Os::Linux));
    assert!(!xz("v0.11.16", Os::Linux));
    assert!(!xz("v0.13.0", Os::Linux));
}

#[test]
fn it_waits_for_io_js_2_3_2_on_macos_and_1_0_0_elsewhere() {
    assert!(xz("v1.0.0", Os::Linux));
    assert!(xz("v3.3.1", Os::Linux));
    assert!(!xz("v1.8.4", Os::Darwin));
    assert!(!xz("v2.3.1", Os::Darwin));
    assert!(xz("v2.3.2", Os::Darwin));
    assert!(xz("v3.3.1", Os::Darwin));
}

#[test]
fn it_names_the_extension() {
    assert_eq!(Compression::Xz.extension(), "tar.xz");
    assert_eq!(Compression::Gzip.extension(), "tar.gz");
}
