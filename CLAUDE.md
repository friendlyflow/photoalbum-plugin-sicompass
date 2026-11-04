# Project Instructions

photoalbum-plugin-sicompass is a Store plugin for
[sicompass](https://github.com/friendlyflow/sicompass), usually driven from a sicompass
checkout next to this one (`../sicompass`).

It is a sicompass **plugin process**: a program (`src/main.rs`) built with the SDK's
`plugin` feature, which sicompass starts and talks to over its stdin and stdout. It runs
with the user's rights. `permissions` in `plugin.json` is what it declares, never a sandbox.
The plugin platform is described in `../sicompass/docs/plugin-platform.md`.

- `plugin.json` is the manifest. `"markup": true` is what lets its rows hold real HTML, and
  **that makes every file name this plugin's to escape** (`tags::escape` for text,
  `markup::escape_attribute` for attributes). A test holds a file called `<b>x.jpg`.
- `locales/<lang>.ftl`, every id prefixed `photoalbum-`, in all four languages.
- `Album` is the logic, tested with `cargo test`. `PhotoAlbum` implements
  `sicompass_sdk::plugin::Plugin` on top of it.
- `src/album.css` is the look, compiled in and sent as a `<style>` row at the top of each
  list. It never travels as a separate file.
- A folder is read on a thread of its own, reported through `poll`, because the app waits
  for every call with no deadline.
- Read-only. Keys, in element mode, the big view and playing a video are all the app's.

**Before the first release:** set `minAppVersion` to the first app release that holds the
grids and video (0.2.3 does not, it is only there so a debug build of the app loads this
one), and keep `SDK_TAG` in the workflows at a release tool that knows `markup`, or it is
dropped from `release.json`.

## Environment (Nix)

The toolchain comes from the flake dev shell in [flake.nix](flake.nix). Nothing is
installed system-wide.

- `command -v cargo` once per session. Non-empty: run `cargo ...` directly. Empty: prefix
  every toolchain command with `nix develop -c`.
- Evaluate the flake through `git+file://$PWD`, never a plain path, and always under
  `timeout`.
- The version lives in `plugin.json` and in `[package] version` in `Cargo.toml`. Bump both
  together.
