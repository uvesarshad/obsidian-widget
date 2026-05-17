# Security Policy

  ## Supported Versions

  Only the latest release on the main branch receives security fixes.

  | Version | Supported |
  |---|---|
  | latest | ✅ |
  | older  | ❌ |

  ## Reporting a Vulnerability

  Please do NOT open a public GitHub issue for security vulnerabilities.

  Email: uveskhan234@gmail.com
  Subject: "ob-widget security: <short summary>"

  Include: reproduction steps, affected version, and any proof-of-concept.

  You should receive an acknowledgement within 7 days. A fix and
  coordinated disclosure timeline will follow once the issue is
  confirmed.

  ## Scope

  In-scope:
  - Path traversal via target_file or reminder_tone_path
  - Markdown rendering XSS in NoteView's md() function
  - Tauri IPC command argument injection
  - Settings.json or reminders.json corruption attacks

  Out-of-scope:
  - Issues that require the attacker to have already compromised the
    user's machine
  - Vulnerabilities in upstream Tauri / Svelte / Rust crates (report
    those to the respective projects)

  ---
  .github/ISSUE_TEMPLATE/bug_report.md

  ---
  name: Bug report
  about: Report something that isn't working
  title: "bug: "
  labels: bug
  ---

  ## Describe the bug
  <!-- One or two sentences. -->

  ## Steps to reproduce
  1.
  2.
  3.

  ## Expected behaviour

  ## Actual behaviour

  ## Environment
  - OS:        <!-- e.g. Windows 11 23H2 / macOS Sonoma 14.5 -->
  - ob-widget: <!-- version from installer filename, or "built from <commit>" -->
  - Obsidian:  <!-- version, if relevant -->
  - Vault size: <!-- approximate # of tasks in the target file -->

  ## Logs / screenshots
  <!-- Right-click the widget → Inspect to open dev tools and copy
       any console output. Attach screenshots if visual. -->

  ## Additional context

  ---
  .github/ISSUE_TEMPLATE/feature_request.md

  ---
  name: Feature request
  about: Suggest an idea for ob-widget
  title: "feat: "
  labels: enhancement
  ---

  ## What problem does this solve?
  <!-- The user-facing pain point. Skip the implementation here. -->

  ## Proposed solution

  ## Alternatives considered

  ## Would you be willing to submit a PR?
  - [ ] Yes
  - [ ] Yes, with guidance
  - [ ] No

  ## Additional context

  ---
  .github/ISSUE_TEMPLATE/config.yml

  blank_issues_enabled: false
  contact_links:
    - name: Question or discussion
      url: https://github.com/uvesarshad/ob-widget/discussions
      about: For open-ended questions, use GitHub Discussions instead of an issue.
    - name: Security vulnerability
      url: https://github.com/uvesarshad/ob-widget/blob/main/SECURITY.md
      about: Do NOT file a public issue — see SECURITY.md.

  ---
  .github/PULL_REQUEST_TEMPLATE.md

  ## Summary
  <!-- 1–3 sentences. What does this PR change and why? -->

  ## Type of change
  - [ ] Bug fix
  - [ ] New feature
  - [ ] Refactor (no behaviour change)
  - [ ] Docs only
  - [ ] Build / CI

  ## Related issue
  <!-- "Closes #123" or "Refs #123" -->

  ## How was this tested?
  - [ ] `npm run check` passes
  - [ ] `cargo check --manifest-path src-tauri/Cargo.toml` passes
  - [ ] Manual smoke test of the affected flow
  <!-- Describe the manual steps and the OS you tested on. -->

  ## Docs updated
  <!-- Did you follow the AGENT UPDATE: tags in /docs? List the files you touched. -->
  - [ ] No docs change needed
  - [ ] Updated: docs/...

  ## Screenshots
  <!-- Required for any UI change. Drag images into this box. -->

  ## Checklist
  - [ ] No new environment variables (use settings.json instead)
  - [ ] No code blocks added to /docs files
  - [ ] No /docs file exceeds 200 lines after my changes
  - [ ] Snake_case preserved on shared Settings/Task/Reminder types
  - [ ] Commit messages follow Conventional Commits