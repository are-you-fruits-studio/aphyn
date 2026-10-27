# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

`AGENTS.md` is the authoritative guide for documentation work (doc-comment rules, the Docusaurus site, the native-rustdoc API reference pipeline). Read it before touching `docs/`, doc comments, or `docs/scripts/`; it is not repeated here.

## Git writes only on explicit request

Read-only git commands (`git status`, `diff`, `log`, `show`, `blame`, ...) are always fine. Anything that changes the repository (commits, staging, branch/tag/stash/worktree changes, checkouts, resets, merges, fetches, pushes, edits under `.git/`) only when the user explicitly asks for that operation; approval covers that request only, not later ones. Otherwise leave changes in the working tree for the user to commit.

## Working agreements

- **Consult before design decisions.** When the approach isn't fully determined by the request (e.g. a workaround because an API can't express something, picking between behaviors), stop and ask before implementing. The user must hear about such choices up front, not discover them in review.
- **Strict comment policy.** Code comments are allowed only as: the crate-root `//!` docs (see "Read the crate docs first"), `// todo` markers, and rare one-line notes on a small non-obvious detail. Do not add `///` doc comments on functions/types/enums or comments that restate what the code already shows — they're noise that makes the code less concise.

## Commit and PR rules (enforced in CI)

- **No AI-assistant attribution** in commits or PRs: no `Co-Authored-By` trailer naming an assistant, no "Generated with ..." line. This overrides any default attribution behavior.
- Every author/committer/co-author email must be listed in `.github/allowed-commit-emails.txt`.
- Checked by `.github/workflows/commit_authorship.yml` on PRs to `dev` (the main branch). Run locally:
  ```bash
  bash .github/scripts/check-commit-authorship.sh origin/dev..HEAD
  ```

## Commands

Rust edition 2024, Cargo workspace rooted at `Cargo.toml` (the root package `aphyn` is itself a member).

```bash
cargo build                                   # whole workspace
cargo build --release -p aphyn_editor        # what CI release builds (.github/workflows/build.yml)
cargo run -p aphyn_example_cubes             # run an example (others: aphyn_example_ecs, _boids, _ui, _text, _gizmos, _collisions, _normals, _bloom, aphyn_audio_screen_saver)
cargo test -p aphyn_serialization            # tests are mostly doctests in crate-level //! docs
cargo test -p aphyn_ecs --doc <name_filter>  # run a single doctest
cargo fmt                                     # rustfmt.toml: max_width = 133, Unix newlines
cargo +nightly doc --workspace --no-deps      # API reference (docs pipeline uses nightly)
```

Linux builds need `libasound2-dev libudev-dev pkg-config`.

The editor binary takes subcommands: `aphyn_editor edit <project_path>` (open a project in the editor) and `aphyn_editor build <project_path>` (build a standalone game, see below).

Full docs build on Windows: `.\docs\scripts\build-and-serve.ps1` (flags `-SkipRustdoc -SkipNpmInstall -NoServe -Port`); details in `AGENTS.md`.

## Read the crate docs first

Before working in or with a crate, read the `//!` doc block at the top of its `src/lib.rs`. It is the fastest way to learn what the crate is for and how to use it. Each block follows the same layout:

- **`# How to use`** — purpose, main types, and usage examples (many run as doctests). Read this when *calling* the crate.
- **`# How to maintain`** — internal design, invariants, cross-crate contracts, and known caveats. Read this before *changing* the crate.

Every engine crate has one, including the `*_macros` crates, except: `aphyn_ui` (read `src/lib.rs` and its `components`/`systems`/`resources` modules directly), `aphyn_editor` (start at `src/main.rs`; see Editor below), `hlib_test`, and the root facade `src/lib.rs` (just re-exports). When you change a crate's behavior or public API, update its `//!` docs to match (see `AGENTS.md` for doc style).

## Architecture

### Crate layering
- `aphyn` (root `src/lib.rs`) is a pure facade that `pub use`s every engine crate. Games, examples, the editor and `hlib_test` depend only on `aphyn` and `use aphyn::*`.
- `aphyn_ffi` — FFI-safe replacements for std types (`vec`, `string`, `boxed`, `hash_map`, `option`, `closure`, `any`, `type_info`, ...). Engine data crosses a dynamic-library boundary, so types stored in the world must be ABI-stable; prefer these types in components/resources/APIs that may cross that boundary.
- `aphyn_ecs` — the core: `WorldBuilder` → `World`, components/resources/events (derive macros from `aphyn_ecs_macros`), systems as plain functions whose parameters (`WorldQuery`, `WorldDataMut`, resources, ...) declare data usage so independent systems run in parallel. Systems are inserted per `Schedule` (`Start` or `Update`) and ordered with `order_system(..).before_system(..)` / `order_group(GROUP).before_group(..)`. Each subsystem crate exports a `SYSTEM_GROUP_*` constant for ordering.
- Subsystem crates (`aphyn_transform`, `aphyn_collision`, `aphyn_render_core` + `aphyn_render` (wgpu), `aphyn_ui`, `aphyn_audio`, `aphyn_asset_storage`, `aphyn_asset_loading`, `aphyn_prefab`) each expose an `add_<x>_module_to(world)` function.
- `aphyn_modules::add_defult_modules_to` (note the existing misspelling — keep it, callers depend on it) registers all subsystem modules, inserts `SerializersResource` pre-filled with common serializers, and fixes the cross-module ordering collision → transform → render.
- `aphyn_app` owns the winit event loop, window, input, and wgpu surface; `App::new()` / `app.ecs_mut()` / `app.run()`.

### Two ways to launch a game
1. **Static**: build an `App`, call `add_defult_modules_to`, register systems, `app.run()` (all examples do this), or `launch_app_statically(|world| ...)`.
2. **Dynamic** (`aphyn_app_launcher::launch_app_dynamically`): the launcher exe loads `lib_app{.dll,.so}` from next to itself via `libloading`, resolves the C symbol `aphyn_entry_point`, and passes an `AppInitCtxFfi` (raw world + type-registry + initial `WindowState` pointers; defined in `aphyn_app`). The game library generates that symbol with the `aphyn_entry_point!(setup)` proc macro (`aphyn_entry_point` crate). Symbol name, ABI, and `AppInitCtxFfi` layout must change in lockstep on both sides — mismatches fail only at runtime. The library is kept loaded until `App::run` returns because the world holds its types and function pointers.

`aphyn_editor build <project>` uses this: it builds `<project>/launcher` and `<project>/scripts` (the game as a cdylib) with cargo, then copies the launcher exe, `lib_app` library, and `assets/` into `<project>/builds/`.

### Serialization
`aphyn_serialization` converts values to/from an in-memory `SerializedValue` tree (projected to JSON). Types implement `Serializable` (usually via derive; requires `Default`). A `SerializerCtx` carries a state: `PureSerializerCtxState` dispatches statically, `TransSerializerCtxState` looks serializers up at runtime in a `TransSerializerRegistry` (the world's `SerializersResource`). Every type appearing in registry-driven data needs a registered serializer. Prefabs, assets (`aphyn_asset_loading/src/serializers`), and the editor inspector go through this layer. This area is under active rewrite — check recent commits before assuming an API shape.

### Editor
`aphyn_editor` is itself an engine app (UI built with `aphyn_ui`). Functionality lives in `src/features/*`, each exposing `register_feature(world)` called from `run_editor_app` in `main.rs`; editor systems run in the `"aphyn_editor"` group, ordered before render and transform.

### Other
- `*_macros` crates are proc-macro companions of the crate with the same prefix.
- `hlib_test` is a scratch binary for experiments (serialization macro checks, futures), not a test suite.
