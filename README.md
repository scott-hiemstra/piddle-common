# piddle-common

Shared Rust crates for the PIDdle family of resource viewers:

| App | Repo | What it shows |
|---|---|---|
| PIDdle | [scott-hiemstra/piddle](https://github.com/scott-hiemstra/piddle) | Local Linux processes |
| kpiddle | [scott-hiemstra/kpiddle](https://github.com/scott-hiemstra/kpiddle) | Kubernetes pods |
| piddle-net | [scott-hiemstra/piddle-net](https://github.com/scott-hiemstra/piddle-net) | Processes on remote machines (in design) |

Nothing here knows where items come from (`/proc`, a Kubernetes API server,
a remote agent). Code moves in only once a second app needs it.

| Crate | What it is | Depends on |
|---|---|---|
| `piddle-encode` | CPU → color and memory → size mappings, number and time formatting, `sanitize` for untrusted text | libc |
| `piddle-scene` | The bubble scene: easing, physics, grouping, radial tree, camera and hit testing, generic over what a bubble stands for | std only |
| `piddle-gtk` | The GTK 4 bubble canvas widget and GtkSnapshot drawing helpers | gtk4 |

`piddle-encode` and `piddle-scene` never link a GUI toolkit, so the
terminal UIs (which ship as static musl binaries) can use them. Only
`piddle-gtk` links GTK.

## Using it

The crates aren't on crates.io yet. Depend on an exact commit of a release
tag, never a branch, and use the same commit for every crate:

```toml
[workspace.dependencies]
piddle-encode = { git = "https://github.com/scott-hiemstra/piddle-common", rev = "<sha>" } # v0.1.0
```

The consumers carry `scripts/bump-common.sh vX.Y.Z`, which looks up the tag's
commit and rewrites every `rev`.

## Changing shared code

1. Check out the repos side by side (`~/Code/piddle-common`, `~/Code/piddle`,
   `~/Code/kpiddle`) and run `scripts/use-local-common.sh on` in each app.
   It writes a gitignored `.cargo/config.toml` that points the app at
   `../piddle-common`, so edits here rebuild there with no tag or push.
   `scripts/use-local-common.sh off` removes it.
2. CI here builds and tests piddle and kpiddle `main` against every commit
   (the `consumers` job), so a breaking change shows up before a tag.
3. Release: add the changes to the Unreleased section of CHANGELOG.md as
   they land; to release, bump `version` in Cargo.toml (one version for
   every crate), date the section, tag `vX.Y.Z` and push the tag. Then run
   `scripts/bump-common.sh vX.Y.Z` in each app.

Versions are 0.x: a breaking change in any crate bumps the minor version.

## History

These crates were extracted from
[scott-hiemstra/piddle](https://github.com/scott-hiemstra/piddle). Their
earlier history is there, at commit `a85dab6` and before:
`crates/piddle-gui/src/view.rs` (the scene), `crates/piddle-core/src/encode.rs`
(the encodings) and `crates/piddle-gui/src/canvas.rs` (the canvas).

## License

MIT, see [LICENSE](LICENSE).
