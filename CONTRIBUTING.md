# Contributing to ob-widget

Thanks for taking the time to contribute! This document describes how to
set up the project, the conventions we follow, and how to submit changes.

By participating, you agree to abide by our [Code of Conduct](CODE_OF_CONDUCT.md).

---

## Ways to contribute

- 🐛 **Bug reports** — open an issue using the [bug report template](.github/ISSUE_TEMPLATE/bug_report.md).
- 💡 **Feature requests** — open an issue using the [feature request template](.github/ISSUE_TEMPLATE/feature_request.md).
- 📝 **Docs improvements** — fixes to README, files under `/docs`, or inline comments are very welcome.
- 🛠️ **Pull requests** — see the workflow below.
- 🔐 **Security issues** — do NOT file a public issue. See [SECURITY.md](SECURITY.md).

---

## Development setup

### Prerequisites

| Tool | Version |
|---|---|
| Rust | ≥ 1.77 (install via [rustup.rs](https://rustup.rs)) |
| Node.js | ≥ 18 |
| cargo-tauri CLI | ≥ 2 (`cargo install tauri-cli`) |
| WebView2 (Windows) | bundled with Windows 11 |
| Xcode Command Line Tools (macOS) | `xcode-select --install` |

### Clone, install, run

```bash
git clone https://github.com/<your-username>/ob-widget
cd ob-widget
npm install
npm run tauri dev
```

First Rust compile takes ~2 minutes; incremental builds are seconds. Svelte
hot-reloads on save; **Rust changes require restarting `npm run tauri dev`.**

### Useful scripts

| Command | Purpose |
|---|---|
| `npm run tauri dev` | Run the widget with HMR |
| `npm run tauri build` | Produce platform installers |
| `npm run check` | TypeScript + Svelte type-check |
| `cargo check --manifest-path src-tauri/Cargo.toml` | Type-check Rust |

---

## Repository layout

A short tour. For the full map see [docs/architecture/folder-structure.md](docs/architecture/folder-structure.md).

- `src/` — SvelteKit frontend (SPA mode)
- `src-tauri/` — Rust backend, all Tauri commands and OS integrations
- `docs/` — documentation for humans and AI coding agents
- `public/` — README screenshots
- `static/` — assets served by SvelteKit at the web root
- `.github/` — issue / PR templates and the release workflow

---

## Code style

### Rust (src-tauri/)

- Edition 2021. Run `cargo fmt` before committing.
- Tauri commands stay in `src-tauri/src/lib.rs` until the file genuinely
  needs splitting. Don't pre-split.
- Every new field on `Settings` or `Reminder` needs `#[serde(default = "fn")]`
  so older JSON files load. See existing fields for the pattern.
- Errors return `Result<T, String>`. Bubble up rather than `.unwrap()`.
- File reads must be size-capped (existing convention: 1 MB for notes,
  5 MB for custom audio).

### TypeScript / Svelte (src/)

- Svelte 5 runes (`$state`, `$derived`, `$props`). Do **not** use the
  legacy `$:` reactive syntax.
- Field names in `src/lib/types.ts` mirror the Rust `Settings` /
  `Reminder` struct names exactly (snake_case). Don't "fix" them.
- Callback props named `on<verb>` (e.g. `ontoggle`, `onconfirm`) —
  avoid `createEventDispatcher`.
- Keep components in `src/lib/` reusable; page-specific shell logic
  stays in `src/routes/+page.svelte`.

### Docs

- Every file in `/docs` follows the format defined in [AGENTS.md](AGENTS.md):
  header, scope, sections, **Update Triggers**, **Related Docs**.
- No code blocks inside docs files.
- Keep each file under 200 lines. Split into `-part1.md` / `-part2.md`
  if a topic outgrows that.
- When a code change adds or alters something documented, update the
  relevant doc file(s) in the same PR. The list of which docs to touch
  is shown by the `AGENT UPDATE:` tag at the bottom of each section.

---

## Commit messages

We follow a lightweight [Conventional Commits](https://www.conventionalcommits.org/) style:

- `feat: add custom reminder tone support`
- `fix: prevent reminder picker AM/PM trap`
- `docs: clarify snooze flow in reminders module`
- `refactor: extract reminder cleanup helper`
- `chore: bump tauri to 2.x`

Keep the subject line under 72 characters. Add a body for the *why* if
the change is non-obvious.

---

## Pull request workflow

1. **Fork** the repo and create a feature branch:
   `git checkout -b feat/short-name` (or `fix/...`, `docs/...`).
2. **Make your change.** Add or update tests if applicable. Update the
   relevant docs in `/docs` for any behaviour change.
3. **Verify locally:**
   - `npm run check` (TypeScript + Svelte)
   - `cargo check --manifest-path src-tauri/Cargo.toml` (Rust)
   - `npm run tauri dev` and try the affected flow by hand
4. **Commit** with a clear Conventional Commit message.
5. **Push** to your fork and open a **pull request** against `main`.
   Fill in the [PR template](.github/PULL_REQUEST_TEMPLATE.md).
6. Be ready for **review feedback** — small iterative PRs are easier to
   land than one giant PR.

### Before requesting review — checklist

- [ ] `npm run check` passes (0 errors)
- [ ] `cargo check` passes (0 errors)
- [ ] Manual smoke test of the affected flow
- [ ] Docs in `/docs` updated where AGENT UPDATE: tags indicated
- [ ] No new env vars (the app uses `settings.json`, not env)
- [ ] No code blocks added to any `/docs` file
- [ ] No file in `/docs` exceeds 200 lines

---

## Reporting bugs

Open a new issue with the [bug report template](.github/ISSUE_TEMPLATE/bug_report.md).
Useful information:

- OS and version (e.g. Windows 11 23H2, macOS Sonoma 14.5)
- ob-widget version (visible in the installer filename, or `--version`)
- Steps to reproduce
- What you expected vs. what happened
- Console / dev-tools output if relevant (right-click the widget →
  **Inspect** in a dev build to see logs)

---

## Releases

Releases are produced by the maintainers from `main`. The
`.github/workflows/release.yml` workflow builds Windows and macOS
installers when a tag is pushed. See [docs/infra/deployment.md](docs/infra/deployment.md) for details.

---

## Questions

If something in this guide is unclear, please open an issue or start a
discussion. We'd rather answer the same question ten times than have
contributors give up because the docs were ambiguous.

Thanks again for contributing! 🎉
