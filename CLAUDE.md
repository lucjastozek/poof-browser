# CLAUDE.md

Read this fully before changing anything. It explains what the product is, what matters most, and
how the code is organised. `docs/ARCHITECTURE.md` has the per-file map; `docs/PERF-CHANGES.md` has
past performance work.

## 1. What this is

**poof** is a desktop web browser inspired by **Arc**. It is written in Rust on `wry` (the system
WebView, which is WebView2 on Windows) and `tao` (window and event loop). The browser chrome is a
vertical **sidebar** (plain HTML/CSS/JS in `ui/`) running in its own WebView; every tab is another
WebView placed next to it.

Product priorities, in order:

1. **Interaction quality.** The browser must feel designed, fast and predictable. Looks matter, but
   the whole interaction experience matters more: no flicker, no layout jumps, no dead keys, no
   state that is lost or surprising. Every shortcut, drag, hover and focus case should work.
2. **Performance and low RAM.** A browser is open all day. It must start fast, stay light with
   dozens of tabs, and cost nothing while idle.
3. **Cross-platform.** It should run on Windows, macOS and Linux. **Windows is the only supported
   target for now**: write new code so it is not Windows-only unless it has to be.

When these conflict, interaction quality wins over cleverness, and correctness wins over both.

## 2. Product model

Learn these terms; the code and UI use them everywhere.

- **Profile**: an isolated browsing identity with its own cookies, storage, cache and extensions
  (its own WebView2 user-data folder). Has retention settings for tabs, cookies, cache and
  history (`state/model.rs`, `browser/profile_cleanup.rs`).
- **Space**: a named, coloured, icon-tagged workspace inside a profile (like Arc spaces). Many
  spaces can share a profile. Switching spaces swaps which tabs the sidebar shows. Icons are ids
  of the built-in icon set, or a sanitised custom SVG, never emoji.
- **Tab kinds** (`Kind`): **Favorite** (shared by all spaces of a profile, shown as the top grid),
  **Pinned** (belongs to one space, persisted, shown above the divider), **Normal** (the
  unpinned list; ephemeral unless "restore tabs" is on).
- **Folders**: tabs can be grouped in collapsible folders, pinned or loose, ordered together with
  loose tabs.
- **Sidebar**: collapsible; collapsed it peeks open when the pointer hits the screen edge
  (edge-peek), and has a compact width. Rows support drag-to-reorder, drag into folders, context
  menus, and drag-to-split.
- **Split view**: two tabs side by side (horizontal or vertical), with a resizable divider.
- **Tab switcher**: Ctrl+Tab cycles tabs with a visible switcher, committing on key release.
- **Sleeping tabs**: background tabs idle too long release their WebView (and renderer process);
  they stay in the sidebar and reload when selected.
- **DevTools**: docked panel next to the page (F12), with a resize handle and a header, themed to
  match the sidebar.
- **Also**: extensions (unpacked, plus Chrome Web Store install), history view, new-tab page with
  suggestions, auto Picture-in-Picture when leaving a tab with a playing video, HTML fullscreen,
  light/dark theme with per-space accent colour.

Arc is the reference for behaviour questions. When unsure how something should behave, do what Arc
does, then make it simpler.

## 3. Performance rules (RAM and CPU)

These are requirements, not nice-to-haves.

- **Idle means idle.** No permanent timers, no polling, no `setInterval` in pages or the sidebar.
  The main loop sleeps until the next real deadline (`ControlFlow::WaitUntil`). Page scripts react
  to events (history API hooks, `visibilitychange`) rather than poll. A poll is allowed only while
  it is visibly needed (for example DevTools device mode) and must stop with it.
- **Hidden tabs are cheap.** Hidden tabs get the WebView2 LOW memory target unless they play
  audio or show PIP. Long-idle background tabs are released entirely (`browser/maintenance.rs`).
  Don't add work that wakes hidden pages.
- **Coalesce updates.** State saves, sidebar pushes and relayouts are rate-limited
  (`SAVE_MIN_INTERVAL`, `STATE_MIN_INTERVAL`). Never write `state.json` or push the whole sidebar
  state on every event; mark dirty and let the timer flush.
- **Bound everything** that grows: history (`HISTORY_MAX`), favicons (`FAVICON_MAX`), icon lists,
  strings from pages (`clip`). Drop per-tab bookkeeping when its WebView goes away.
- **Cache data once, free what you can.** Large static data (icon pack, DevTools icons) is
  loaded lazily and released when no longer needed. Don't keep big strings alive "just in case".
- **Don't clone big structures** in hot paths, and don't send the full state to the sidebar for a
  small change if a patch will do.
- **Measure, don't guess.** A claimed win needs a before/after number (private working set in
  Task Manager, or time-to-first-paint). Don't add complexity for an unmeasured gain. A change
  that risks startup is not worth a small RAM gain: a browser that opens blank is worse than a
  heavy one.
- Page-injected JavaScript runs in every site; keep it tiny, event-driven and defensive (wrap in
  `try/catch`, never throw into the page, never break a site).

## 4. Interaction quality rules

- **Shortcuts work from anywhere.** Browser shortcuts must work whichever WebView has focus. Page
  focus: WebView2 accelerators in `shortcuts.rs`. Sidebar focus: `ui/scripts/sidebar/shortcuts.js`.
  When you add one, wire both paths, and don't steal keys from text fields.
- **Focus follows intent.** After opening, switching or closing a tab, focus goes where the user
  expects (the page, or the address bar for a new tab). Esc always backs out of the topmost
  thing (menu, dialog, switcher, fullscreen).
- **No flicker, no jumps.** Layout is computed natively in `browser/layout.rs`; resizing, sidebar
  toggles, split changes and fullscreen must not flash the bare window. Keep a solid backdrop
  while content is not painted yet.
- **Sites must behave like they do in Chrome.** Never restyle, hijack or replace a site's own
  UI. In particular fullscreen and video use the site's own player and controls
  (`browser/js/fullscreen.js`); never fullscreen a bare `<video>` as the normal path. Popups,
  auth flows (Google sign-in, Cloudflare challenges) and embedded frames must be left untouched.
- **Never lose user data.** Tabs, spaces, folders and pins survive crashes and force-quits:
  atomic writes (`state/storage.rs`) with a backup, tolerant loading (`#[serde(default)]`, aliases
  for renamed fields), and migrations that never drop unknown data.
- **Fail soft.** A failed WebView2 call, bad message or missing file is logged
  (`log::warn!`) and ignored, not a panic and not a blank window. The browser must always open
  with a usable sidebar and a working first tab.
- **Polish counts.** Hover, active and focus states, animation timing, truncation, empty states
  and error states are part of the feature. Match the existing visual language (tokens in
  `ui/styles/foundation/theme.css`); don't introduce one-off colours, sizes or fonts.
- **Accessibility basics.** Keyboard reachable controls, visible focus, `aria-label` on icon-only
  buttons, respects the theme.

## 5. Cross-platform rules

- Put OS-specific code behind `#[cfg(windows)]` / `#[cfg(not(windows))]` and always provide a
  non-Windows implementation, even if it is a documented no-op (see `attach_native_handlers`,
  `gesture::eval_with_gesture`). Don't let platform code leak into feature files; keep it in
  `browser/platform.rs`, `browser/webview_events.rs`, `browser/shortcuts.rs`, `gesture.rs`.
- Don't hard-code Windows paths, `\\` separators or key codes outside the platform files. Use
  `dirs`, `Path`/`PathBuf`, and logical shortcut names (`"new_tab"`, `"devtools_toggle"`) in
  shared code.
- On macOS the modifier is Cmd, not Ctrl; keep the shortcut table the single place to change that.
- The UI is plain web tech and must stay renderer-agnostic: no WebView2-only APIs in `ui/`.

## 6. How to write the code

### Structure
- **One job per file, small files.** If a file grows past roughly 300 lines or starts to cover two
  topics, split it. Name files after the topic (`folders.rs`, `address-bar.js`), not the kind
  (`utils`, `helpers`, `misc`).
- `src/main.rs` only builds the window and runs the loop. `src/browser/mod.rs` holds the `Browser`
  struct and constructor only; behaviour goes in the topic file as an `impl Browser` block. Child
  modules use `use super::*;` and expose methods `pub(super)`.
- **JS in `.js` files, CSS in `.css` files, HTML in `.html`**, never in Rust strings. Load with
  `include_str!`. Injected page scripts live in `src/browser/js/`, the sidebar in `ui/`.
- Don't name a module `url`: it would shadow the `url` crate.

### Clean code
- Functions do one thing and fit on a screen (aim for under 40 lines). Extract named helpers
  instead of writing comments that explain blocks. Early-return instead of nesting.
- Names say what and why (`toggle_page_fullscreen`, not `do_fs2`). No abbreviations except the
  established ones in this codebase (`wv`, `tab`).
- Magic numbers become named constants (`browser/constants.rs`, `ui/scripts/core/`) with a unit or
  a reason in the doc comment.
- Comments explain *why*, never what. Doc comments (`///`, `//!`) on every module and every
  non-trivial item. Delete dead code; don't comment it out.
- No duplicate logic. If two places need it, extract it (see `write_json_atomically`,
  `eval_with_gesture`).
- Make illegal states unrepresentable: enums over strings and booleans, newtypes over bare ids
  when it prevents mixups.
- Propagate errors with `anyhow` and `?`, log at the boundary. No `unwrap()`/`expect()` on
  anything that depends on the OS, a file, a page or the user. Windows API wrappers must tolerate
  failure.
- Keep `cargo check` and `cargo clippy` warning-free. Run `rustfmt`.
- Tests: pure logic (parsing, sanitising, state migration, message formats, shortcut mapping)
  gets unit tests next to it. Bug fix means a regression test when the logic is testable.

### Rust patterns used here
- **Messages are typed.** Sidebar to browser: `Cmd` (`#[serde(tag = "cmd")]`). Page to browser:
  `PageMsg` (`#[serde(tag = "t")]`). Both in `browser/messages.rs`. To add a command: add the
  variant, handle it in `commands.rs` / `page_messages.rs`, send it from the UI with
  `send({cmd: "..."})`. Keep the JSON shape flat and backward compatible.
- **Untrusted input is sanitised at the edge** (`browser/sanitize.rs`,
  `ui/scripts/views/svg-sanitizer.js`): URLs, colours, names, SVG icons, anything from a page or
  from disk. Pages must never be able to run commands as the sidebar; the empty tab id is
  reserved for the sidebar itself.
- **State changes flow one way**: mutate `Browser` state, mark it dirty, then push to the UI and
  persist (debounced). The sidebar renders from the state it receives; it doesn't own data.
- **Windows-only handlers** talk to WebView2 through `webview2_com` and `windows`. Keep each in a
  small named function (`watch_focus`, `watch_fullscreen`) with logging on failure.

### Frontend (sidebar)
- Plain JS, no framework, no build step. Each script is its own `<script>` tag sharing one global
  scope, so **load order matters** and file-level names are global: avoid collisions. Styles are
  concatenated into one `<style>` tag. Register new files in `STYLES` / `SCRIPTS` in
  `src/browser/sidebar_page.rs`.
- Layout: `ui/styles/{foundation,layout,views}/`, `ui/scripts/{core,sidebar,views}/`. Shared
  helpers in `core/`, sidebar chrome in `sidebar/`, full-page views (start page, settings,
  history, dialogs, space editor) in `views/`.
- Use design tokens (CSS variables) from `foundation/theme.css`; support light and dark theme and
  each space's accent colour.
- Build DOM with the helpers in `core/dom.js`; never inject untrusted strings with `innerHTML`.
- Keep functions small. Existing long ones (`buildSpace`, `buildSettings`, `updateDrag`) are
  known debt: split them when you touch them, don't add to them.
- `ui/icons.js` is generated by `tools/gen-icons.mjs`; don't edit it by hand.

## 7. Verifying changes

- `cargo build`, `cargo test`, `cargo clippy` on Windows, which is the only way to compile the
  `#[cfg(windows)]` code (WebView2 hooks, accelerators, profile cleanup, gesture injection).
  A non-Windows `cargo check` passes without compiling those parts; say so when that is all you
  could run.
- For UI changes, load the sidebar page and check light and dark theme, collapsed and expanded
  sidebar, narrow and wide widths, and keyboard-only use.
- For anything touching tabs, layout, fullscreen, focus or startup, test by hand: open, switch,
  close, reopen, split, collapse, fullscreen (from page focus *and* sidebar focus), restart.
  **The app must always start into a working window.** If startup is touched, test a cold start
  with an empty profile and with an existing state file.
- For performance work, compare memory with the same tabs open before and after, and confirm that
  idle CPU is ~0%.

## 8. Known gotchas

- **Fullscreen**: use the site's own fullscreen button (even if hidden), then its F key, and only
  then the player container. The page must own focus, and the call must carry a user gesture
  (`gesture::eval_with_gesture`). The window itself goes borderless via WebView2's
  `ContainsFullScreenElementChanged`.
- **PIP** also needs a user gesture; it only pops out videos the user started themselves
  (`pip/track.js`), so autoplaying previews and ads are never popped out.
- **DevTools** must stay docked; undocked it opens a separate window the browser never shows.
- A profile whose last WebView is gone loses its WebView2 environment, so its extensions must be
  loaded again (`ext_done`).
- Anti-bot and auth iframes (Cloudflare Turnstile, Google sign-in) get no injected scripts at all.
- `poof-icon.svg` is embedded by `app_icon.rs` and must exist at the project root.
- **One user-data folder, one set of browser args.** wry builds a new WebView2 environment for every
  WebView, and WebView2 refuses two environments with different args on the same folder. Every
  WebView built on `ui_ctx` (sidebar, popup bar, DevTools frontend) must pass `WEBVIEW2_UI_ARGS`,
  otherwise it silently fails to load (the DevTools panel opened empty because of this).
- **Importing from Arc** (`src/import/`, `browser/arc_import.rs`): files are read on a thread and
  returned as plain data; the main thread creates profiles, spaces and logins and never replaces
  anything that exists. Passwords: `Login Data` (SQLite, copied first, Arc locks it while running)
  is decrypted with the AES key in `Local State`, which DPAPI protects, so it only works on
  Windows as the same user. `v20` (app-bound) passwords are counted and skipped. Arc's
  `StorableSidebar.json` is undocumented: the parser skips what it does not recognise.
