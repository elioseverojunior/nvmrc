//! The mirror fixture the real `nvm.sh` was run against, for tests of the
//! remote listing.

use crate::domain::index::{Release, parse_index};
use crate::domain::version::Flavor;

#[must_use]
pub fn index_text(rows: &[(&str, &str)]) -> String {
    let header = "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity";
    let body: Vec<String> = rows
        .iter()
        .map(|(version, lts)| {
            format!(
                "{version}\t2023-01-01\tlinux-x64\t10.0.0\t11.0.0\t1.0.0\t1.3\t3.0\t100\t{lts}\t-"
            )
        })
        .collect();
    format!("{header}\n{}\n", body.join("\n"))
}

#[must_use]
pub fn node_releases() -> Vec<Release> {
    parse_index(
        &index_text(&[
            ("v21.2.0", "-"),
            ("v21.1.0", "-"),
            ("v20.10.0", "Iron"),
            ("v20.9.0", "Iron"),
            ("v18.19.0", "Hydrogen"),
            ("v18.18.0", "Hydrogen"),
            ("v16.20.2", "Gallium"),
            ("v14.21.3", "Fermium"),
            ("v4.9.1", "Argon"),
            ("v4.0.0", "-"),
            ("v0.12.18", "-"),
            ("v0.10.48", "-"),
        ]),
        Flavor::Node,
    )
}

#[must_use]
pub fn iojs_releases() -> Vec<Release> {
    parse_index(
        &index_text(&[
            ("v3.3.1", "-"),
            ("v3.0.0", "-"),
            ("v2.5.0", "-"),
            ("v1.0.0", "-"),
        ]),
        Flavor::IoJs,
    )
}
