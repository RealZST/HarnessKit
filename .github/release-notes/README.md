# Release Notes

Hand-written highlights for each release.

## How it works

Before tagging a release `vX.Y.Z`, drop a file `vX.Y.Z.md` here containing
the Highlights section (and anything else you want at the top of the
release body).

The Release workflow (`.github/workflows/release.yml`, `Generate changelog`
step) prepends this file's content to the auto-generated PR list, so the
content shows up in:

- The GitHub release page body
- `latest.json`'s `notes` field — i.e. the in-app updater dialog

If the file doesn't exist, the workflow falls back to just the auto PR
list (existing behavior).

## Example

`v1.5.0.md`:

```markdown
## ✨ Highlights

- **Faster Check Updates** — repos are now cloned in parallel
- **Manual source binding** — bind any locally-installed skill to its
  upstream GitHub repo from the detail panel

---
```

The trailing `---` separates highlights from the auto PR list.

## Release flow

1. Write `.github/release-notes/v<tag>.md`
2. `bash scripts/bump-version.sh <tag>`
3. `git add -A && git commit -m "release: v<tag>"`
4. `git push`
5. `git tag v<tag> && git push origin v<tag>`
6. Workflow auto-generates draft release with highlights prepended
7. Publish the draft on GitHub
