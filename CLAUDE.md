# RightPeek — agent conventions

`PLAN.md` is the source of truth (goals, budgets, architecture, phases). Read the
relevant section before working; do not re-plan what it already decides.

## Language
- Chat with the user: **Persian (Farsi)**.
- Everything in the repo (code, identifiers, comments, commits, docs, skills): **English**.

## Layout
- `src/` — viewer frontend: vanilla TS + Vite, no framework.
- `src-tauri/src/core/` — pure logic, no OS calls, unit-tested (TDD).
- `src-tauri/src/platform/` — OS backends implementing `core::capture::SelectionProvider`.
- `src-tauri/src/lib.rs` — app wiring only.
- `src-tauri/capabilities/` — least-privilege Tauri permissions.

## Rules
- Work phase by phase (PLAN.md §8); stop for user approval at the end of each phase.
- `core/` is test-first. Platform code is validated with the capture probe + matrix.
- Never install low-level keyboard/mouse hooks (`WH_KEYBOARD_LL`, `WH_MOUSE_LL`).
- Never widen Tauri capabilities beyond what a feature needs.
- Ask the user before: new dependencies not named in PLAN.md, architecture changes,
  anything irreversible (publishing, signing, pushing tags).
- Conventional Commits; one phase = one branch/PR.
- Windows capture work: use the `windows-text-capture` skill.

## Navigation (token economy)
- Use graft first: `graft_repo_map`, `graft_find_code`, `graft_file_api`,
  `graft_trace_calls` (before changing a symbol). Read only needed line ranges.
- Never read `graft/` wholesale; never paste whole files or full logs into chat.

## Phase gate
Run from repo root:
`cargo fmt --manifest-path src-tauri/Cargo.toml --check`,
`cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`,
`cargo test --manifest-path src-tauri/Cargo.toml`, `npm run build`, `graft check`;
from Phase 2 also measure the budgets in PLAN.md §2.
