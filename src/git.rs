//! The git boundary — the only module that runs a subprocess.
//!
//! Exactly two calls ever happen: read-only branch-label resolution at
//! startup, and `git add -- <file>` after a successful save.

use std::path::Path;
use std::process::Command;

/// The one error text a staging failure ever produces.
const STAGE_FAILED: &str = "resolved file saved but not staged";

/// The current branch name; empty result (detached HEAD, mid-rebase) falls
/// back to the short SHA; any failure returns `None` so the caller shows the
/// literal marker label instead. Read-only.
pub fn current_branch_label() -> Option<String> {
    let current = git_stdout(&["branch", "--show-current"])?;
    if current.is_empty() {
        branch_display_fallback(git_stdout(&["rev-parse", "--short", "HEAD"]))
    } else {
        Some(current)
    }
}

/// `git add -- <path>` after a successful write. Idempotent, never retried.
/// Any failure returns the fixed error string; the file stays saved either way.
pub fn stage(path: &Path) -> Result<(), String> {
    let arguments = stage_command_args(path);
    let status = Command::new("git")
        .args(&arguments)
        .status()
        .map_err(|_| STAGE_FAILED.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(STAGE_FAILED.to_string())
    }
}

/// Display label for the current side: a marker label that names no branch
/// (`HEAD` or empty) shows the resolved branch name, falling back to `local`;
/// every other marker label is shown verbatim. The remote side never goes
/// through this.
pub fn branch_label_display(marker_label: &str, resolved: Option<&str>) -> String {
    if label_needs_branch(marker_label) {
        resolved
            .map(str::to_string)
            .unwrap_or_else(|| "local".to_string())
    } else {
        marker_label.to_string()
    }
}

/// Does this marker label name no branch, so displaying it needs the resolved
/// branch? Exactly the rule `branch_label_display` applies: the trimmed label
/// is empty or is `HEAD`. The one question the startup git call answers.
pub fn label_needs_branch(marker_label: &str) -> bool {
    let trimmed = marker_label.trim();
    trimmed.is_empty() || trimmed == "HEAD"
}

/// The argument vector of the staging call, kept pure so tests never exec.
pub fn stage_command_args(path: &Path) -> Vec<String> {
    vec![
        "add".to_string(),
        "--".to_string(),
        path.to_string_lossy().into_owned(),
    ]
}

/// Detached-HEAD fallback for `git rev-parse --short HEAD` output: trimmed,
/// or `None` when git produced nothing usable.
pub fn branch_display_fallback(sha: Option<String>) -> Option<String> {
    sha.map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

/// Runs one git subcommand and returns its trimmed stdout on success, `None`
/// on a non-zero exit, a missing binary or non-UTF-8 output.
fn git_stdout(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|text| text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn branch_label_display_resolves_head_marker() {
        assert_eq!(branch_label_display("HEAD", Some("feature/x")), "feature/x");
        assert_eq!(branch_label_display("", Some("abc1234")), "abc1234");
        assert_eq!(branch_label_display("   ", Some("main")), "main");
        assert_eq!(branch_label_display("HEAD", None), "local");
        assert_eq!(branch_label_display("", None), "local");
    }

    #[test]
    fn branch_label_display_keeps_plain_labels() {
        assert_eq!(branch_label_display("feature/y", Some("main")), "feature/y");
        assert_eq!(branch_label_display("mine", None), "mine");
        assert_eq!(branch_label_display("theirs", Some("main")), "theirs");
    }

    #[test]
    fn label_needs_branch_cases() {
        assert!(label_needs_branch(""), "an empty label is a branch");
        assert!(label_needs_branch("  "), "so is a blank one");
        assert!(label_needs_branch("HEAD"));
        assert!(label_needs_branch(" HEAD "), "trimmed first");
        assert!(
            !label_needs_branch("ours"),
            "a real label is shown verbatim"
        );
        assert!(!label_needs_branch("feature/x"));
        assert!(!label_needs_branch("head"), "case-sensitive, like git");
    }

    #[test]
    fn stage_command_args_are_the_git_add_command() {
        let path = PathBuf::from("conflicted.txt");
        assert_eq!(
            stage_command_args(&path),
            vec!["add", "--", "conflicted.txt"]
        );
    }
}
