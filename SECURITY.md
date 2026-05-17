# Security Policy

## Supported Versions

Only the latest release on the `main` branch receives security fixes.

| Version | Supported |
|---|---|
| latest | ✅ |
| older  | ❌ |

## Reporting a Vulnerability

**Please do NOT open a public GitHub issue for security vulnerabilities.**

Report privately by email:

- **To:** uveskhan234@gmail.com
- **Subject:** `ob-widget security: <short summary>`

Include in your report:

- A clear description of the vulnerability
- Steps to reproduce (proof-of-concept if available)
- The affected version (installer filename or commit SHA)
- Your assessment of the impact
- Optional: any suggested fix or mitigation

You should receive an acknowledgement within **7 days**. A coordinated
disclosure timeline and patch plan will follow once the issue is
confirmed and triaged.

## Scope

The following are in scope for this project:

- Path traversal via `Settings.target_file` or `Settings.reminder_tone_path`
- HTML / markdown injection in the `md()` function inside
  `src/lib/NoteView.svelte` (XSS via the `{@html}` consumer)
- Tauri IPC command argument injection or capability bypass
- Corruption attacks against `settings.json` or `reminders.json`
- Local privilege escalation through the autostart entry on
  Windows / macOS
- Resource exhaustion via crafted markdown files, audio files, or
  reminder payloads (file-size caps exist; bypasses are in scope)

## Out of scope

- Issues that require the attacker to have already compromised the
  user's machine or local user account
- Vulnerabilities in upstream Tauri, Svelte, Rust crates, or system
  libraries — please report those to the respective projects
- Social-engineering attacks against end users
- Missing code signing on installers (a known, documented limitation)

## Safe harbor

We will not pursue legal action against researchers who:

- Report vulnerabilities in good faith via the channel above
- Avoid privacy violations, data destruction, and service disruption
- Give us a reasonable window to respond and patch before public
  disclosure

Thank you for helping keep ob-widget and its users safe.
