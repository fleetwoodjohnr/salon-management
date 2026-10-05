# Architecture

## Stack

| Layer | Choice | Why |
|---|---|---|
| Desktop shell | Tauri 2 (Rust) | Small native packages for Linux, Windows, macOS using the system webview; privileged work stays in Rust |
| Interface | React 19, TypeScript, Vite | Mature, typed, fast builds |
| Components | Mantine 9 (+ dates, charts, notifications, spotlight, modals), mantine-datatable | One consistent accessible component set: theming, focus handling, light/dark |
| Charts | Recharts via `@mantine/charts` | Free, themeable |
| Calendar | react-big-calendar (+ drag-and-drop addon) with dayjs | Resource (staff) day view, drag to reschedule |
| Dashboard grid | react-grid-layout 2 | Drag/resize/save widget layouts |
| Data | SQLite (bundled via `rusqlite`), STRICT tables, foreign keys, WAL | No database server; one file per workspace |
| Money | `rust_decimal` | Exact base-10 arithmetic |
| HTTP | `reqwest` (rustls) | Provider calls happen only in Rust, never from the webview |
| Secrets | `keyring` 4 (Secret Service / Keychain / Credential Manager) | API keys never touch the database or frontend |
| Fonts | Figtree and Bodoni Moda (OFL), bundled | Offline; no font CDN |

All dependencies are MIT, Apache-2.0, BSD-style, ISC or OFL licensed.

## Layout

```
src-tauri/src/
  lib.rs          plugin setup, command registration
  workspace.rs    workspace registry, open/close/restore, transaction helpers
  backup.rs       zip backups with checksummed manifest, verified restore
  attachments.rs  receipts stored by content hash
  csvio.rs        CSV import (mapping → preview → commit), exports, portable export
  demo.rs         fictional demo data (also the large-dataset test)
  gazetteer.rs    offline ZIP/city centres from the bundled Census Gazetteer (src-tauri/data/)
  domain/         pure calculations, no IO: money, units, profile, costing, pricing,
                  inventory (moving average), sale (totals/tax/refunds), market (statistics)
  db/             SQL per area: core, profiles, inventory, services, pricing (what-if), tax,
                  appointments, clients, estimates, sales, expenses, market, reports, dashboards
  commands/       thin Tauri command handlers (deserialize → validate → db/domain)
  providers/      HTTP plumbing (cache, quotas, retries, de-duplication, keyring) and
                  WA DOR, CDTFA, Census, BLS, Overpass clients
src-tauri/migrations/   001…006 SQL migrations (embedded in the binary)
src/
  api/            invoke wrapper, query helpers, TypeScript mirrors of the Rust types
  app/            shell, navigation, routes, theme
  components/     shared pieces (decimal input, breakdown, page header, unsaved-changes tracking, undo)
  features/       one folder per area (inventory, services, pricing, tax, calendar, sales, …)
  lib/            display formatting (string-based decimal rounding), units, dates
e2e/              real-app end-to-end scripts (WebDriver against the built binary)
```

## Principles

- **The Rust core is the single source of truth.** The interface sends inputs and displays decimal
  strings. Live previews (profile rates, service estimates, landed cost, what-if pricing, sale
  totals) are computed by preview commands, debounced as you type.
- **Domain code is pure** (`domain/`), so financial rules are unit-tested without a database.
- **Every write runs in a transaction** (`AppState::tx`); a failed step rolls back everything.
- **History is append-only where money is concerned**: profile versions, recipe versions, tax rate
  and taxability versions, the stock ledger, refunds. Finalized sales snapshot all figures used.
- **Validated boundary**: commands deserialize into typed structs that reject unknown fields, then
  validate business rules. Errors return `{kind, message, field}`, so forms show messages next to the
  right field.
- **Least privilege**: the webview gets only `core:default`, window close, file dialogs, clipboard
  write and opening `https://` links. No shell, filesystem or HTTP plugin access from JavaScript.
  Strict content security policy (`default-src 'self'`, IPC only). File reads/writes happen in Rust
  on paths the person picked in a native dialog. Tauri's `freezePrototype` is off because the calendar's
  date library assigns to its own prototype (see PROGRESS.md).
- **Responsiveness**: commands are `async` and run on the async runtime, off the UI thread. Network
  calls release the database lock while waiting.

## Workspaces

Each workspace is a folder with its own `salon.db` and `attachments/`. The registry
(`workspaces.json`) lists them and remembers the last one opened. The demo workspace is a separate
workspace of kind `demo`, with a permanent banner; demo and real data can't mix. Workspaces are not
designed for simultaneous use from several computers.

## Startup and migrations

Opening a workspace: SQLite pragmas (foreign keys, WAL, full sync, busy timeout) → if the data
version is older, back up first, then apply each pending migration in its own transaction and run a
foreign-key check → daily automatic backup if due. Data from a newer app version is refused.

## Interface patterns

- Persistent left navigation grouped by task; `Ctrl+K` command palette; `Ctrl+1…9` jump to pages;
  `Ctrl+N` new item on list pages; `Ctrl+S` save in editors.
- Unsaved-changes protection: in-app navigation and window close both ask first.
- Undo toasts for reversible actions (appointment status, archiving a client); corrections to money and
  stock use explicit reversals instead.
- Sale drafts autosave; they have no effect on stock until finalized.
- Printing (receipts, estimates, reports, dashboard) uses the system print dialog via `window.print()`
  with print styles; "Print to File" produces a PDF.
