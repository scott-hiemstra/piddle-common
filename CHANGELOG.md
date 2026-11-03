# Changelog

All notable changes to the piddle-common crates. Every crate shares one
version; entries say which crate they touch.

## [Unreleased]

### Added

- `scripts/use-local-common.sh APP_DIR on|off`: points an app at this
  checkout through a marked `[patch]` block in its `.cargo/config.toml`,
  keeping its other settings and restoring its Cargo.lock on `off`. It was
  copied in each app; the apps now call this one, and the CI `consumers` job
  uses it too. It refuses what it can't take apart safely (damaged markers,
  an app's own patch table, an old-style file with additions), escapes
  paths for TOML and replaces the file atomically. A script, not crate
  code, so no release is needed for it.
- `scripts/test-use-local-common.sh`, run by a new CI job: every case above
  against a throwaway app, under paths with spaces, quotes and backslashes.
- `piddle-scene`: clusters. `Scene::set_cluster_of` maps a group's info to
  a cluster key; in the Grouped arrangement, groups with the same key are
  pulled together and packed as one circle, so no other group ends up
  inside, and `Scene::clusters` gives each cluster's outer hull and totals
  for drawing. The camera keeps the outer hulls in view. PIDdle uses it for
  the containers of one Compose project or pod. Additive: apps that don't
  call it are unchanged.

## [0.1.1] - 2026-11-01

### Added

- `piddle-gtk::details`: the building blocks of the GUIs' details windows,
  so PIDdle and kpiddle stop copying them: the shared CSS (`CSS`,
  `add_css`), a details `window` with Escape to close, `stat_card`,
  `set_text`, and `confirm_typed`, the type-the-name confirmation for risky
  actions (the action stays disabled until the name is typed exactly;
  `name_matches` is the rule, with tests).

## [0.1.0] - 2026-11-01

### Added

- First release, extracted from PIDdle 0.4.2 so PIDdle and kpiddle share
  code instead of copying it.
- `piddle-encode`: CPU color and memory radius scales, `fmt_bytes`,
  `fmt_rate`, `fmt_cpu`, `fmt_clock`, `sanitize` for untrusted text, and
  `mix` for blending colors.
- `piddle-scene`: the bubble scene, generic over an `Item` trait, with free,
  grouped and tree arrangements.
- `piddle-gtk`: the bubble canvas widget (drawing through a `Paint` trait)
  and GtkSnapshot helpers (`ring`, `card`, `text`, `layout`, `font`, `rgba`,
  `glib_escape`, ...).
