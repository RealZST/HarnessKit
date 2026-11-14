# Suggestions & Backlog

## What's Next
- Push branch `feat/dedicated-repositories` to remote `origin` and mark PR #186 ready for review (`gh pr ready 186`).
- Monitor remote CI workflows (`PR Checks`, `Website Checks`, `CodeQL`) on GitHub Actions.

## Issues & Room for Improvement
- Linux host missing GTK 2.0 / `gio-2.0` pkg-config dependencies prevents running `cargo test --workspace` or `cargo clippy --workspace` (fails on `hk-desktop`). Always target `-p hk-core -p hk-cli -p hk-web` on environments without desktop GUI toolchains.

## Reusable Takeaways
- **Local Repo Cache Deployment**: When updating repository-backed skills, deploy from the synced local cache (`~/.harnesskit/repos/<repo_id>`) via `install_from_repo_item` to avoid slow, redundant remote `git clone` network roundtrips.
- **Git Credential Scrubbing**: Pass `http.extraHeader` tokens via environment variables `GIT_CONFIG_COUNT=1`, `GIT_CONFIG_KEY_0=http.extraHeader`, and `GIT_CONFIG_VALUE_0=...` rather than `-c` CLI arguments so credentials never appear in `/proc/<pid>/cmdline`.
- **Test Concurrency (`kit_env`)**: Tests altering `HARNESSKIT_HOME` or `HOME` via RAII guards must annotate with `#[serial_test::serial(kit_env)]` to prevent parallel test races against kit service tests.
