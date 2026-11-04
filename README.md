# photoalbum-plugin-sicompass

*The Sicompass photo album: your pictures and videos as a grid.*

This plugin is part of [Sicompass](https://github.com/friendlyflow/sicompass), a
keyboard-first, accessibility-first way to use your entire computer.

It shows a folder of pictures and videos as a grid of three columns, each with its file
name below it. Subfolders are rows you enter with Right. Moving onto the grid, your screen
reader says how many pictures and videos it holds. Press i to go to the first one and a to
go to the last, move with the arrows, and press Enter or d to see one big or to play it.
Escape goes back.

It only reads. It never changes, moves or deletes a file. It reads JPEG, PNG and WebP
pictures and MP4, WebM, MKV and MOV videos, and skips everything else.

## Settings

- **photo folder**: the folder to show. A relative path is under your home folder, and the
  default is `Pictures`.
- **sort order**: by name, or by date with the oldest first.

## Install

In Sicompass, open store, then programs, and press Enter on install next to photo album.
The Store checks the release's signature before installing it, and keeps it up to date.

To install a build of your own instead, copy `plugin.json`, the built `plugin` program
(`plugin.exe` on Windows) and `locales/` into a folder named `photoalbum` in the Sicompass
plugins folder (`~/.config/sicompass/plugins/` on Linux, `~/Library/Application
Support/sicompass/plugins/` on macOS) and restart Sicompass.

## Building from source

```bash
nix develop -c cargo build
nix develop -c cargo test
```

## License

GPL-3.0-only. See [LICENSE](LICENSE).
