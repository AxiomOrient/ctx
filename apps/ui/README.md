UI (Tauri + Svelte 5)
=====================

Summary
- Svelte 5 (runes) SPA loaded by Tauri window.
- SSOT state via a single writable store in `src/lib/ssot.ts`.
- IPC thin wrappers in `src/lib/ipc.ts` call Tauri commands.

Dev
- Install deps: npm i
- Run: npm run dev
- Build Tauri (Rust) separately with `--features ui_tauri,ui_ipc,ui_render_rust`.

Security
- HTML from backend is sanitized server-side; UI renders with `{@html ...}`.
- No direct filesystem access from frontend; all via Tauri commands.

