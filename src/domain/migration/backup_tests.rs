use super::{backup_name, latest_backup};

#[test]
fn the_backup_name_carries_a_compact_utc_stamp() {
    assert_eq!(
        backup_name(".zshrc", 1_791_110_040),
        ".zshrc.nvmrc-backup-20261004T103400Z"
    );
    assert_eq!(
        backup_name(".bashrc", 0),
        ".bashrc.nvmrc-backup-19700101T000000Z"
    );
}

#[test]
fn the_latest_backup_is_the_greatest_stamp_of_that_file() {
    let candidates: Vec<String> = [
        ".zshrc",
        ".zshrc.nvmrc-backup-20261004T103400Z",
        ".zshrc.nvmrc-backup-20261005T000000Z",
        ".zshrc.nvmrc-backup-20250101T000000Z",
        ".zshrc.nvmrc-backup-20991231T235959",
        ".zshrc.nvmrc-backup-2099123IT235959Z",
        ".zshrc.nvmrc-backup-20991231T235959Z.old",
        ".zshrc.nvmrc-backup-",
        ".zshrc2.nvmrc-backup-20991231T235959Z",
        "x.zshrc.nvmrc-backup-20991231T235959Z",
        ".bashrc.nvmrc-backup-20991231T235959Z",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(
        latest_backup(".zshrc", &candidates),
        Some(".zshrc.nvmrc-backup-20261005T000000Z")
    );
    assert_eq!(
        latest_backup(".bashrc", &candidates),
        Some(".bashrc.nvmrc-backup-20991231T235959Z")
    );
    assert_eq!(latest_backup(".profile", &candidates), None);
}

#[test]
fn equal_stamps_pick_the_lexicographically_greater_name() {
    let name = ".zshrc.nvmrc-backup-20261004T103400Z".to_owned();
    let candidates = vec![name.clone(), name.clone()];
    assert_eq!(latest_backup(".zshrc", &candidates), Some(name.as_str()));
    assert_eq!(latest_backup(".zshrc", &[]), None);
}
