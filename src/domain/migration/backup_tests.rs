use super::{backup_name, latest_backup};

/// 2026-10-04 10:34:00 UTC, the time of every lookup below.
const NOW: i64 = 1_791_110_040;

#[test]
fn the_backup_name_carries_a_compact_utc_stamp() {
    assert_eq!(
        backup_name(".zshrc", 1_791_110_040, 0),
        ".zshrc.nvmrc-backup-20261004T103400Z"
    );
    assert_eq!(
        backup_name(".bashrc", 0, 0),
        ".bashrc.nvmrc-backup-19700101T000000Z"
    );
}

#[test]
fn a_backup_taken_in_a_second_already_used_gets_a_sequence_number() {
    assert_eq!(
        backup_name(".zshrc", 1_791_110_040, 1),
        ".zshrc.nvmrc-backup-20261004T103400Z-1"
    );
    assert_eq!(
        backup_name(".zshrc", 1_791_110_040, 12),
        ".zshrc.nvmrc-backup-20261004T103400Z-12"
    );
}

#[test]
fn the_latest_backup_is_the_greatest_stamp_of_that_file() {
    let candidates: Vec<String> = [
        ".zshrc",
        ".zshrc.nvmrc-backup-20261003T103400Z",
        ".zshrc.nvmrc-backup-20261004T103400Z",
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
        latest_backup(".zshrc", &candidates, NOW),
        Some(".zshrc.nvmrc-backup-20261004T103400Z")
    );
    assert_eq!(
        latest_backup(".bashrc", &candidates, NOW),
        Some(".bashrc.nvmrc-backup-20991231T235959Z")
    );
    assert_eq!(latest_backup(".profile", &candidates, NOW), None);
}

#[test]
fn equal_stamps_pick_the_lexicographically_greater_name() {
    let name = ".zshrc.nvmrc-backup-20261004T103400Z".to_owned();
    let candidates = vec![name.clone(), name.clone()];
    assert_eq!(
        latest_backup(".zshrc", &candidates, NOW),
        Some(name.as_str())
    );
    assert_eq!(latest_backup(".zshrc", &[], NOW), None);
}

#[test]
fn within_one_second_the_greatest_sequence_number_is_the_latest() {
    let stamp = ".zshrc.nvmrc-backup-20261004T103400Z";
    let candidates: Vec<String> = ["", "-1", "-9", "-10", "-x", "-", "-01", "-2-3"]
        .into_iter()
        .map(|suffix| format!("{stamp}{suffix}"))
        .chain([".zshrc.nvmrc-backup-20261004T103359Z-99".to_owned()])
        .collect();
    let expected = format!("{stamp}-10");
    assert_eq!(
        latest_backup(".zshrc", &candidates, NOW),
        Some(expected.as_str())
    );
    let plain = [stamp.to_owned(), format!("{stamp}-x")];
    assert_eq!(latest_backup(".zshrc", &plain, NOW), Some(stamp));
}

#[test]
fn a_backup_dated_after_now_only_wins_when_it_is_the_only_one() {
    let future = ".bashrc.nvmrc-backup-20991231T000000Z".to_owned();
    let past = ".bashrc.nvmrc-backup-20261001T000000Z".to_owned();
    let both = [future.clone(), past.clone()];
    assert_eq!(latest_backup(".bashrc", &both, NOW), Some(past.as_str()));
    let alone = [future.clone()];
    assert_eq!(latest_backup(".bashrc", &alone, NOW), Some(future.as_str()));
}
