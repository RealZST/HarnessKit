//! Running git on the user's behalf, where nobody is there to answer a prompt.

use crate::HkError;
use std::io::{IsTerminal, Read, Seek, SeekFrom};
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

/// How long one git command may take before it counts as hung.
const TIMEOUT: Duration = Duration::from_secs(300);

/// Run git and return its output. It cannot ask for a credential, and gives
/// up on a stalled HTTP transfer or after `TIMEOUT`, so no caller waits
/// forever. In `dir`, git only ever works on that checkout: left to search,
/// it would take a broken checkout for part of a repository around it.
pub(crate) fn run(dir: Option<&Path>, args: &[&str]) -> Result<String, HkError> {
    run_within(TIMEOUT, dir, args)
}

fn run_within(timeout: Duration, dir: Option<&Path>, args: &[&str]) -> Result<String, HkError> {
    // Output goes to files: pipes would have to be drained while waiting.
    let (mut stdout, mut stderr) = (tempfile::tempfile()?, tempfile::tempfile()?);
    let mut cmd = crate::command("git");
    if std::io::stdin().is_terminal() && ssh_is_default(dir) {
        // Started from a terminal, as `hk serve` is, ssh would ask there
        // about an unknown host key or a passphrase, and wait for an answer.
        cmd.env("GIT_SSH_COMMAND", "ssh -oBatchMode=yes");
    }
    cmd.env("GIT_TERMINAL_PROMPT", "0")
        .args([
            "-c",
            "http.lowSpeedLimit=1000",
            "-c",
            "http.lowSpeedTime=30",
        ])
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .stdin(Stdio::null())
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?);
    if let Some(dir) = dir {
        cmd.current_dir(dir).env("GIT_DIR", dir.join(".git"));
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| HkError::CommandFailed(format!("Failed to run git: {e}")))?;
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(HkError::CommandFailed(format!(
                "git {} timed out after {} seconds",
                args[0],
                timeout.as_secs()
            )));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let read = |file: &mut std::fs::File| -> Result<String, HkError> {
        let mut bytes = Vec::new();
        file.seek(SeekFrom::Start(0))?;
        file.read_to_end(&mut bytes)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    };
    if !status.success() {
        let stderr = read(&mut stderr)?;
        let reason: Vec<&str> = stderr
            .lines()
            .filter(|line| !line.starts_with("Cloning into"))
            .collect();
        return Err(HkError::CommandFailed(format!(
            "git {} failed: {}",
            args[0],
            reason.join("\n").trim()
        )));
    }
    Ok(read(&mut stdout)?.trim().to_string())
}

/// Whether git connects with plain `ssh` in `dir`: the user has not chosen a
/// command of their own there, which ours would replace.
fn ssh_is_default(dir: Option<&Path>) -> bool {
    let mut config = crate::command("git");
    config.args(["config", "--get", "core.sshCommand"]);
    if let Some(dir) = dir {
        config.current_dir(dir).env("GIT_DIR", dir.join(".git"));
    }
    let configured = config.output().is_ok_and(|output| output.status.success());
    !configured
        && std::env::var_os("GIT_SSH_COMMAND").is_none()
        && std::env::var_os("GIT_SSH").is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_git_command_that_hangs_is_stopped() {
        let hang = ["-c", "alias.hang=!sleep 5", "hang"];
        let started = Instant::now();
        let err = run_within(Duration::from_millis(200), None, &hang).unwrap_err();
        assert!(err.to_string().contains("timed out"), "{err}");
        assert!(started.elapsed() < Duration::from_secs(3));
    }
}
