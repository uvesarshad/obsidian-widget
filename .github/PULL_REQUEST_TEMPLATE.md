## Summary
<!-- 1–3 sentences. What does this PR change and why? -->

## Type of change
- [ ] Bug fix (non-breaking change that fixes an issue)
- [ ] New feature (non-breaking change that adds functionality)
- [ ] Breaking change (would cause existing behaviour to change)
- [ ] Refactor (no behavioural change)
- [ ] Docs only
- [ ] Build / CI / tooling

## Related issue
<!-- "Closes #123" / "Fixes #123" / "Refs #123" -->

## How was this tested?
- [ ] `npm run check` passes (0 errors)
- [ ] `cargo check --manifest-path src-tauri/Cargo.toml` passes (0 errors)
- [ ] Manual smoke test of the affected flow
<!-- Describe the manual steps and the OS(es) you tested on. -->

## Docs updated
<!--
Per the AGENT UPDATE: tags in /docs, did you touch any documentation?
List the files you updated, or check "no docs change needed".
-->
- [ ] No docs change needed
- [ ] Updated: docs/...

## Screenshots / recordings
<!-- Required for any UI change. Drag images or short videos here. -->

## Checklist
- [ ] My code follows the project style (see CONTRIBUTING.md)
- [ ] Commit messages follow Conventional Commits
  (`feat:`, `fix:`, `docs:`, `refactor:`, `chore:`)
- [ ] No new environment variables introduced
  (the app uses settings.json / reminders.json instead)
- [ ] Snake_case preserved on shared Settings / Task / Reminder fields
  (do not "fix" them to camelCase)
- [ ] No code blocks added to any file under `/docs`
- [ ] No file under `/docs` exceeds 200 lines after my changes
- [ ] New Tauri commands (if any) are registered in the
  `invoke_handler!` macro at the bottom of `src-tauri/src/lib.rs`
  AND documented in `docs/api/server-actions.md`
- [ ] New OS-level integrations (if any) have their capability added
  to `src-tauri/capabilities/default.json` AND are documented in
  `docs/api/external-services.md`
