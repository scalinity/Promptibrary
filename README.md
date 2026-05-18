# Promptibrary

Agentic-coding operator console for Claude Code. A Tauri 2.x desktop app for organizing, launching, and observing Claude Code sessions from a library of typed prompt profiles.

## Status

This is V1, built in layers. Source of truth lives in `docs/SPEC.md`. The layered build plan lives in `~/Documents/Obsidian Vault/Promptibrary/Build Plan.md`.

Current layer: **L0 — Foundations** (in progress).

## Stack

- **Frontend:** React 19, TypeScript, Vite 8, Tailwind v4 (CSS-first via `@theme`), shadcn/ui
- **Backend:** Tauri 2.x, Rust, SQLite via sqlx, sqlite-vec, fastembed, portable-pty, git2
- **Editor:** CodeMirror 6 with custom variable-ref `Decoration.replace` widgets
- **Terminal:** xterm.js 5.5 with fit/web-links/search/unicode11 addons

## Development

```bash
pnpm install
rustup update stable
pnpm tauri:dev
```

Node ≥ 20.19, pnpm ≥ 10.

## License

UNLICENSED — internal project.
