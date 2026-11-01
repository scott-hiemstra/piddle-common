# Changelog

All notable changes to the piddle-common crates. Every crate shares one
version; entries say which crate they touch.

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
