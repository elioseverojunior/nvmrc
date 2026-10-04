use super::unified_diff;

/// `count` numbered lines, `1\n2\n...`.
fn numbered(count: usize) -> String {
    (1..=count).map(|number| format!("{number}\n")).collect()
}

#[test]
fn equal_texts_have_no_diff() {
    assert_eq!(unified_diff(".zshrc", "a\nb\n", "a\nb\n"), "");
    assert_eq!(unified_diff(".zshrc", "", ""), "");
}

#[test]
fn an_insertion_shows_three_lines_of_context() {
    let diff = unified_diff(
        ".bashrc",
        "a\nb\nc\nd\ne\nf\ng\n",
        "a\nb\nc\nd\nX\ne\nf\ng\n",
    );
    assert_eq!(
        diff,
        "--- a/.bashrc\n+++ b/.bashrc\n@@ -2,6 +2,7 @@\n b\n c\n d\n+X\n e\n f\n g\n"
    );
}

#[test]
fn a_replaced_line_is_a_deletion_then_an_insertion() {
    let diff = unified_diff("p", "1\n2\n3\n", "1\nTWO\n3\n");
    assert_eq!(
        diff,
        "--- a/p\n+++ b/p\n@@ -1,3 +1,3 @@\n 1\n-2\n+TWO\n 3\n"
    );
}

#[test]
fn distant_changes_make_two_hunks() {
    let old = numbered(20);
    let new = old.replace("\n2\n", "\nB\n").replace("\n18\n", "\nR\n");
    assert_eq!(
        unified_diff("p", &old, &new),
        "--- a/p\n+++ b/p\n\
@@ -1,5 +1,5 @@\n 1\n-2\n+B\n 3\n 4\n 5\n\
@@ -15,6 +15,6 @@\n 15\n 16\n 17\n-18\n+R\n 19\n 20\n"
    );
}

#[test]
fn changes_whose_contexts_touch_share_one_hunk() {
    let old = numbered(20);
    let new = old.replace("\n2\n", "\nB\n").replace("\n9\n", "\nN\n");
    let diff = unified_diff("p", &old, &new);
    assert!(
        diff.starts_with("--- a/p\n+++ b/p\n@@ -1,12 +1,12 @@\n"),
        "{diff}"
    );
    assert_eq!(diff.matches("@@ -").count(), 1);
}

#[test]
fn a_missing_final_newline_is_marked() {
    assert_eq!(
        unified_diff("p", "a\nb", "a\nc"),
        "--- a/p\n+++ b/p\n@@ -1,2 +1,2 @@\n a\n-b\n\\ No newline at end of file\n+c\n\\ No newline at end of file\n"
    );
    assert_eq!(
        unified_diff("p", "a", "a\n"),
        "--- a/p\n+++ b/p\n@@ -1 +1 @@\n-a\n\\ No newline at end of file\n+a\n"
    );
}

#[test]
fn an_empty_side_starts_at_line_zero() {
    assert_eq!(
        unified_diff("p", "", "a\n"),
        "--- a/p\n+++ b/p\n@@ -0,0 +1 @@\n+a\n"
    );
    assert_eq!(
        unified_diff("p", "a\nb\n", ""),
        "--- a/p\n+++ b/p\n@@ -1,2 +0,0 @@\n-a\n-b\n"
    );
}

#[test]
fn huge_files_fall_back_to_one_whole_file_hunk() {
    let old = numbered(20_001);
    let new = old.replace("\n20001\n", "\nlast\n");
    let diff = unified_diff("p", &old, &new);
    assert!(
        diff.starts_with("--- a/p\n+++ b/p\n@@ -1,20001 +1,20001 @@\n-1\n"),
        "{}",
        &diff[..80]
    );
    assert_eq!(
        diff.lines().filter(|line| line.starts_with('-')).count(),
        20_002
    );
    assert_eq!(
        diff.lines().filter(|line| line.starts_with('+')).count(),
        20_002
    );
}

#[test]
fn a_large_rewrite_stays_cheap_and_correct() {
    let old: String = (0..3000).map(|number| format!("old {number}\n")).collect();
    let new: String = (0..3000).map(|number| format!("new {number}\n")).collect();
    let diff = unified_diff("p", &old, &new);
    assert!(diff.starts_with("--- a/p\n+++ b/p\n@@ -1,3000 +1,3000 @@\n-old 0\n"));
    assert_eq!(diff.lines().count(), 3 + 6000);
}
