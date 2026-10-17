use super::pins::{PinError, parse_pins};
use super::{NvmrcContent, process_content};
use crate::domain::tool::Tool;
use crate::domain::version::Version;

fn version(text: &str) -> Version {
    text.parse().unwrap()
}

#[test]
fn reads_the_three_pins() {
    let pins = parse_pins("20.11.1\nnpm=10.9.0\npnpm=9.12.0\n").unwrap();
    assert_eq!(pins.npm, Some(version("10.9.0")));
    assert_eq!(pins.yarn, None);
    assert_eq!(pins.pnpm, Some(version("9.12.0")));
    assert_eq!(pins.get(Tool::Npm), Some(version("10.9.0")));
    assert_eq!(pins.get(Tool::Node), None);
    let tools: Vec<Tool> = pins.iter().map(|(tool, _)| tool).collect();
    assert_eq!(tools, [Tool::Npm, Tool::Pnpm]);
    assert_eq!(
        pins.package_manager(),
        Some((Tool::Pnpm, version("9.12.0")))
    );
}

#[test]
fn a_partial_pin_is_rejected_with_the_file_and_line() {
    for line in ["pnpm=9.12", "pnpm=v9.12.0", "pnpm=latest", "npm=1.2.3-rc.1"] {
        let error = parse_pins(&format!("20\n{line}\n")).unwrap_err();
        assert_eq!(error, PinError::NotExact { line: line.into() });
    }
}

#[test]
fn yarn_and_pnpm_together_are_rejected() {
    let error = parse_pins("yarn=4.5.0\npnpm=9.12.0\n").unwrap_err();
    assert_eq!(error, PinError::YarnAndPnpm);
}

#[test]
fn comments_and_spaces_are_ignored() {
    let pins = parse_pins("# tools\n npm = 10.9.0 # pinned\r\n").unwrap();
    assert_eq!(pins.npm, Some(version("10.9.0")));
}

#[test]
fn other_keys_are_ignored() {
    let pins = parse_pins("node=20\nfoo=bar\n=1\n").unwrap();
    assert_eq!(pins.iter().count(), 0);
}

#[test]
fn process_content_still_returns_the_bare_version() {
    let contents = "20.11.1\nnpm=10.9.0\npnpm=9.12.0\n";
    assert_eq!(
        process_content(contents),
        NvmrcContent::Version("20.11.1".into())
    );
}
