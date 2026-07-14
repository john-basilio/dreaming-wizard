# Dreaming Wizard

Dreaming Wizard (also called D-wiz) is a narrative authoring tool inspired by [Twine](https://twinery.org/).

While the Twine's functions are mostly for pure narrative, Dreaming Wizard aims to provide more support in authoring narrative stories on the context of game development, rather than just pure storytelling.

## Features

- **Story canvas** — an infinite, pannable and zoomable canvas of story nodes (passages). Choice links between nodes are drawn as arrows, so the branching structure of the story reads directly off the canvas.
- **Story node editor** — clicking a node opens its editor: the node's content is an ordered list of typed blocks, laid out like a messaging thread.
  - *Narration* — narrator prose; sits on the right with a fixed narrator avatar.
  - *Dialogue* — a character's spoken line; sits on the left with the character's avatar and an inline speaker picker fed by the Characters tab (characters are referenced by id, so renames propagate everywhere).
  - *Choice* — player-facing options, each optionally linking to another node (never the node being edited); these links are the arrows on the canvas.
  - *Directive* — a `command` + `argument` pair intended for the game engine rather than the reader (e.g. `play_music` / `tavern_theme`).
  - *Note* — an author-only comment, visually tinted, meant to never reach players.

  Prose blocks collapse to a few lines while browsing and expand into an in-bubble Save/Cancel editor when clicked. Blocks reorder by dragging (the bubble itself, or the pill that appears on hover) with a live drop-position hint, and delete via a hover-only button. New blocks come from the toolbar at the top of the editor.
- **Characters tab** — the project's cast: name, portrait, comment, and description, edited in place.
- **Find (Ctrl+F)** — searches node titles *and* node content (prose, choice labels, directives) as well as character names, then jumps to and highlights the match.
- **Projects** — File → New/Load/Save (Ctrl+S) keeps everything in a `project.json` inside the project's folder. Unsaved changes are flagged in the title bar and guarded against accidental exit.

## Installation

A [justfile](./justfile) is included by default for the [casey/just][just] command runner.

- `just` builds the application with the default `just build-release` recipe
- `just run` builds and runs the application
- `just install` installs the project into the system
- `just vendor` creates a vendored tarball
- `just build-vendored` compiles with vendored dependencies from that tarball
- `just check` runs clippy on the project to check for linter warnings
- `just check-json` can be used by IDEs that support LSP

## Translators

[Fluent][fluent] is used for localization of the software. Fluent's translation files are found in the [i18n directory](./i18n). New translations may copy the [English (en) localization](./i18n/en) of the project, rename `en` to the desired [ISO 639-1 language code][iso-codes], and then translations can be provided for each [message identifier][fluent-guide]. If no translation is necessary, the message may be omitted.

## Packaging

If packaging for a Linux distribution, vendor dependencies locally with the `vendor` rule, and build with the vendored sources using the `build-vendored` rule. When installing files, use the `rootdir` and `prefix` variables to change installation paths.

```sh
just vendor
just build-vendored
just rootdir=debian/dreaming-wizard prefix=/usr install
```

It is recommended to build a source tarball with the vendored dependencies, which can typically be done by running `just vendor` on the host system before it enters the build environment.

## Developers

Developers should install [rustup][rustup] and configure their editor to use [rust-analyzer][rust-analyzer]. To improve compilation times, disable LTO in the release profile, install the [mold][mold] linker, and configure [sccache][sccache] for use with Rust. The [mold][mold] linker will only improve link times if LTO is disabled.

[fluent]: https://projectfluent.org/
[fluent-guide]: https://projectfluent.org/fluent/guide/hello.html
[iso-codes]: https://en.wikipedia.org/wiki/List_of_ISO_639-1_codes
[just]: https://github.com/casey/just
[rustup]: https://rustup.rs/
[rust-analyzer]: https://rust-analyzer.github.io/
[mold]: https://github.com/rui314/mold
[sccache]: https://github.com/mozilla/sccache
