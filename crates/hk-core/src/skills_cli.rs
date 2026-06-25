//! Delegation to the external `skills` CLI (https://github.com/vercel-labs/skills).
//!
//! Skills installed by that CLI live in `~/.agents/skills/<name>` and are tracked
//! in `~/.agents/.skill-lock.json` (recording each skill's `skillFolderHash` +
//! source). When HarnessKit updates such a skill itself, it rewrites the canonical
//! files but cannot update that lockfile's hash, so the two tools drift.
//!
//! Instead we hand the *update* to the `skills` CLI, which rewrites the files AND
//! its own lockfile in one step. We never parse or write `.skill-lock.json` — the
//! tool that owns the format owns the bookkeeping. When the CLI is unavailable the
//! caller falls back to HarnessKit's own deploy (the lockfile, if it was synced
//! from another machine, self-heals the next time the CLI runs there).
//!
//! Delete is intentionally NOT delegated: HK's delete is agent/path-granular,
//! whereas `skills remove` always removes from every agent. Removing a per-agent
//! symlink doesn't touch the CLI's canonical `~/.agents` copy (no lockfile drift),
//! so native removal is both safe and more precise.

use std::process::Command;

use crate::error::HkError;
use crate::models::ConfigScope;

/// Scope restriction passed to the CLI so a Global op never touches a project
/// copy and vice versa. Externally-managed skills are detected only from the
/// global `~/.agents/.skill-lock.json`, so in practice this is always `-g` today
/// (HarnessKit doesn't read the CLI's per-project `skills-lock.json`); the
/// `Project` arm is kept for symmetry should that change.
fn scope_flag(scope: &ConfigScope) -> &'static str {
    match scope {
        ConfigScope::Global => "-g",
        ConfigScope::Project { .. } => "-p",
    }
}

/// Build the `skills update` argument list for a single named skill, run
/// non-interactively (`-y`) and scope-restricted. Pure, so it is unit-tested.
fn build_update_args(skill_name: &str, scope: &ConfigScope) -> Vec<String> {
    vec![
        "update".to_string(),
        skill_name.to_string(),
        scope_flag(scope).to_string(),
        "-y".to_string(),
    ]
}

/// The launchers tried in order: a `skills` binary on PATH, else `npx --yes
/// skills` (which fetches it on demand).
const LAUNCHERS: [(&str, &[&str]); 2] = [("skills", &[]), ("npx", &["--yes", "skills"])];

/// Run the `skills` CLI with `args` via the first available launcher. Returns
/// `Ok(true)` when a launcher ran and exited 0, `Ok(false)` when NO launcher is
/// installed (caller falls back to its own path), and `Err` when a launcher ran
/// but exited non-zero.
///
/// Caveat: the `skills` CLI exits 0 even when an update changes nothing (e.g.
/// upstream unreachable) — it only prints a failure line. So `Err` here catches
/// launch/IO failures, not "the update did nothing". HarnessKit's follow-up
/// rescan is the source of truth for what actually changed on disk.
fn run(args: &[String]) -> Result<bool, HkError> {
    run_with(&LAUNCHERS, args)
}

/// [`run`] with an injectable launcher list, so the launch/fallback control flow
/// can be unit-tested with real shell utilities.
fn run_with(launchers: &[(&str, &[&str])], args: &[String]) -> Result<bool, HkError> {
    for (program, lead) in launchers {
        let mut cmd = Command::new(program);
        cmd.args(*lead).args(args);
        match cmd.output() {
            Ok(out) if out.status.success() => return Ok(true),
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                return Err(HkError::CommandFailed(format!(
                    "`{program}` skills command failed: {}",
                    stderr.trim()
                )));
            }
            // This launcher isn't installed — try the next one.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(HkError::CommandFailed(e.to_string())),
        }
    }
    Ok(false)
}

/// Update one externally-managed skill via the CLI. See [`run`] for the return
/// contract (`Ok(false)` = CLI unavailable → caller falls back).
pub fn try_update(skill_name: &str, scope: &ConfigScope) -> Result<bool, HkError> {
    run(&build_update_args(skill_name, scope))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_update_args_pins_the_cli_invocation() {
        // The exact command string is the most breakage-prone part of delegation.
        // Manifest skills are global-only, so `-g` is what's exercised in practice.
        assert_eq!(
            build_update_args("tdd", &ConfigScope::Global),
            vec!["update", "tdd", "-g", "-y"]
        );
    }

    #[test]
    fn run_with_no_launcher_available_returns_false() {
        // No launcher on PATH → Ok(false) so the caller falls back to its own path.
        let launchers: [(&str, &[&str]); 1] = [("hk-no-such-binary-xyz", &[])];
        assert!(!run_with(&launchers, &["update".into()]).unwrap());
    }

    #[test]
    fn run_with_falls_through_missing_launcher_to_present_one() {
        // First launcher missing, second present and exits 0 → Ok(true).
        let launchers: [(&str, &[&str]); 2] = [("hk-no-such-binary-xyz", &[]), ("true", &[])];
        assert!(run_with(&launchers, &[]).unwrap());
    }

    #[test]
    fn run_with_nonzero_exit_is_error_not_fallthrough() {
        // A launcher that runs but exits non-zero surfaces as Err — it must NOT
        // fall through to the next launcher (don't mask a real failure).
        let launchers: [(&str, &[&str]); 2] = [("false", &[]), ("true", &[])];
        assert!(run_with(&launchers, &[]).is_err());
    }
}
