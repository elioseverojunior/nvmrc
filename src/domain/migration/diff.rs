//! A unified diff (`diff -u` format) of two texts, line by line.
//!
//! The common head and tail are matched first; what lies between them is
//! aligned with a longest-common-subsequence table. Two guards keep the cost
//! bounded: a file of more than [`MAX_LINES`] lines is shown as one
//! whole-file hunk (every old line removed, every new line added), and a
//! middle whose table would exceed [`MAX_CELLS`] cells is shown as removed
//! then added (still a correct diff, just not a minimal one).

use std::ops::Range;

/// The lines of context around each change.
const CONTEXT: usize = 3;
/// Above this many lines (either side), one whole-file hunk.
const MAX_LINES: usize = 20_000;
/// The largest LCS table built (4 bytes a cell: 16 MB).
const MAX_CELLS: usize = 4_000_000;

/// What a step of the edit script does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    Same,
    Removed,
    Added,
}

/// One step, with the 0-based positions in both texts before it.
#[derive(Debug, Clone, Copy)]
struct Step {
    change: Change,
    old_index: usize,
    new_index: usize,
}

/// The edit script being built, with the current positions.
#[derive(Debug, Default)]
struct Script {
    steps: Vec<Step>,
    old_index: usize,
    new_index: usize,
}

impl Script {
    fn push(&mut self, change: Change, times: usize) {
        for _ in 0..times {
            self.steps.push(Step {
                change,
                old_index: self.old_index,
                new_index: self.new_index,
            });
            self.old_index += usize::from(change != Change::Added);
            self.new_index += usize::from(change != Change::Removed);
        }
    }
}

/// The unified diff from `old` to `new`, both shown as `path` (`--- a/<path>`,
/// `+++ b/<path>`, or the path as it is when absolute), with 3 lines of context and the hunks merged when their
/// contexts touch; a line without final newline is followed by
/// `\ No newline at end of file`. Empty when the texts are equal. A range of
/// one line is written without its count (`@@ -1 +1,2 @@`), an empty range
/// as the line before it with count 0 (`-0,0`), as GNU diff does.
#[must_use]
pub fn unified_diff(path: &str, old: &str, new: &str) -> String {
    if old == new {
        return String::new();
    }
    let old_lines: Vec<&str> = old.split_inclusive('\n').collect();
    let new_lines: Vec<&str> = new.split_inclusive('\n').collect();
    let mut script = Script::default();
    if old_lines.len() > MAX_LINES || new_lines.len() > MAX_LINES {
        script.push(Change::Removed, old_lines.len());
        script.push(Change::Added, new_lines.len());
    } else {
        edit_script(&old_lines, &new_lines, &mut script);
    }
    let mut diff = if path.starts_with('/') {
        format!("--- {path}\n+++ {path}\n")
    } else {
        format!("--- a/{path}\n+++ b/{path}\n")
    };
    for range in hunks(&script.steps) {
        render_hunk(&mut diff, &script.steps[range], &old_lines, &new_lines);
    }
    diff
}

/// The steps from `old` to `new`: common head, aligned middle, common tail.
fn edit_script(old: &[&str], new: &[&str], script: &mut Script) {
    let head = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let tail = old[head..]
        .iter()
        .rev()
        .zip(new[head..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    script.push(Change::Same, head);
    let old_middle = &old[head..old.len() - tail];
    let new_middle = &new[head..new.len() - tail];
    if old_middle.len().saturating_mul(new_middle.len()) > MAX_CELLS {
        script.push(Change::Removed, old_middle.len());
        script.push(Change::Added, new_middle.len());
    } else {
        align(old_middle, new_middle, script);
    }
    script.push(Change::Same, tail);
}

/// The steps from `old` to `new` along a longest common subsequence,
/// removals before additions.
fn align(old: &[&str], new: &[&str], script: &mut Script) {
    let width = new.len() + 1;
    let mut table = vec![0_u32; (old.len() + 1) * width];
    for row in (0..old.len()).rev() {
        for column in (0..new.len()).rev() {
            table[row * width + column] = if old[row] == new[column] {
                table[(row + 1) * width + column + 1] + 1
            } else {
                table[(row + 1) * width + column].max(table[row * width + column + 1])
            };
        }
    }
    let (mut row, mut column) = (0, 0);
    while row < old.len() && column < new.len() {
        let change = if old[row] == new[column] {
            Change::Same
        } else if table[(row + 1) * width + column] >= table[row * width + column + 1] {
            Change::Removed
        } else {
            Change::Added
        };
        script.push(change, 1);
        row += usize::from(change != Change::Added);
        column += usize::from(change != Change::Removed);
    }
    script.push(Change::Removed, old.len() - row);
    script.push(Change::Added, new.len() - column);
}

/// The step ranges of the hunks: each run of changes with its context, runs
/// at most `2 * CONTEXT` unchanged lines apart sharing one hunk.
fn hunks(steps: &[Step]) -> Vec<Range<usize>> {
    let mut runs: Vec<Range<usize>> = Vec::new();
    let changes = steps
        .iter()
        .enumerate()
        .filter(|(_, step)| step.change != Change::Same);
    for (index, _) in changes {
        match runs.last_mut() {
            Some(run) if index - run.end <= 2 * CONTEXT => run.end = index + 1,
            _ => runs.push(index..index + 1),
        }
    }
    runs.into_iter()
        .map(|run| run.start.saturating_sub(CONTEXT)..(run.end + CONTEXT).min(steps.len()))
        .collect()
}

/// Appends the hunk of `steps` (header and lines) to `diff`.
fn render_hunk(diff: &mut String, steps: &[Step], old: &[&str], new: &[&str]) {
    let Some(first) = steps.first() else {
        return;
    };
    let count = |kept: Change| steps.iter().filter(|step| step.change != kept).count();
    diff.push_str(&format!(
        "@@ -{} +{} @@\n",
        hunk_range(first.old_index, count(Change::Added)),
        hunk_range(first.new_index, count(Change::Removed))
    ));
    for step in steps {
        let (marker, line) = match step.change {
            Change::Same => (' ', old[step.old_index]),
            Change::Removed => ('-', old[step.old_index]),
            Change::Added => ('+', new[step.new_index]),
        };
        diff.push(marker);
        diff.push_str(line);
        if !line.ends_with('\n') {
            diff.push_str("\n\\ No newline at end of file\n");
        }
    }
}

/// `start,count` of a hunk header (`start` 0-based, shown 1-based).
fn hunk_range(start: usize, count: usize) -> String {
    match count {
        0 => format!("{start},0"),
        1 => format!("{}", start + 1),
        _ => format!("{},{count}", start + 1),
    }
}
