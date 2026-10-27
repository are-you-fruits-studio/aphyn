---
name: aphyn-docs-generation
description: >-
  Writes high-value rustdoc for a single target crate/module in the aphyn workspace.
  All documentation lives on the crate main page (a //! overview split into "How to use" and
  "How to maintain"); item-level /// comments are not used. Edits only the main page — no other
  source file is modified. Invoke as /aphyn-docs-generation <target>.
disable-model-invocation: true
---

# aphyn-docs-generation

Write rustdoc for **one explicitly chosen** part of the `aphyn` workspace. Run
incrementally — one crate/module/folder/file per invocation — so docs grow in reviewable
chunks. Never document the whole workspace in one run.

The argument after the command is the **target**: a crate name, a module path, a folder, or a
single file.

## Guiding principle (read this first)

**The code explains itself; the main page explains the crate.** Item-level doc comments are a
liability — they restate what the signature already says and rot the moment the code changes. A
reviewer rejected a "document everything" pass with *"isn't it obvious?"* on almost every line.

So this skill produces **exactly one kind of documentation**: the crate-level `//!` overview on
the main page. That is the whole deliverable.

- **Only the main page is edited.** This skill writes (or extends) the `//!` overview on the
  crate main page and modifies **no other file**. Do not remove existing `///` item docs from
  other files, do not edit or reformat their developer comments, do not make any incidental change
  there — every other source file stays exactly as it is. The whole change is confined to one file.
- **Do not write item-level `///` comments** on functions, structs, enums, traits, constants,
  type aliases, fields, variants, or methods — public or private.
- Let names, types, and signatures carry the per-item meaning. If an item is so unclear that it
  *needs* a sentence, that is a signal to rename it, not to comment it.

## 1. Resolve the target

Determine the scope of `.rs` files:

- **Crate name** (e.g. `aphyn_collision`) → the whole crate under `crates/<dir>/src/` (dir = crate name without the `aphyn_` prefix).
- **Folder / module path** (e.g. `crates/ecs/src/behavior`) → that folder, recursively.
- **Single file** (e.g. `crates/ecs/src/world.rs`) → just that file.

Find the **owning crate** (nearest parent package `Cargo.toml`) for the verification step.

If no target was provided, **stop and ask**. Do not document the whole workspace.

Announce the resolved scope (files + owning crate) before editing.

## 2. Read the project rules

Read [AGENTS.md](../../../AGENTS.md): *Sources of Truth*, *Style*, *Examples*, *What to Avoid*.
Key constraints: **code is the source of truth — never invent behavior**; clear technical
English; concrete verbs ("owns", "borrows", "registers", "schedules"); no filler.

## 3. The crate overview is the only deliverable

Write (or extend) a `//!` block at the top of the crate root (`lib.rs`). Do **not** open it with
a bullet list of the crate's types — rustdoc generates that navigation already. The page has
three parts: a one- or two-sentence summary, then two top-level sections.

```rust
//! # aphyn_<name>
//!
//! One or two sentences on what problem this crate solves.
//!
//! # How to use
//!
//! For someone *using* the engine. Public API only, never internals.
//!
//! # How to maintain
//!
//! For someone *developing/maintaining* this crate: how it works inside.
```

**The opening summary is a pure overview, nothing more.** It states *what problem the crate
solves* and *what it is for* — the role it plays in the engine. It must **not** describe technical
implementation (threads, data structures, FFI, control flow) or *how to apply* the crate (entry
points, registration, API steps) — those belong in *How to maintain* and *How to use* respectively.
Keep it to one or two plain sentences a reader skims to decide whether the crate is relevant; do
not name specific functions or types.

### Structure: small sub-sections, not a wall of text

Break each section into small sub-sections, one per topic or use-case, under a `####` (h4)
heading — `#`/`##` render large and underlined and would compete with the `How to use` /
`How to maintain` titles. Never run two unrelated examples together without a heading between them.

### How to use

Show *what functionality the crate offers* and *how a user invokes it*: the **public API a user
touches directly**, through the idiomatic entry points the engine actually expects (check the
codebase for the real path — a subsystem is usually pulled in via the engine's *default modules*
registration, not by calling a crate's internal `add_*_module_to` directly). Order sub-sections by
how common the use-case is — most common first (Unity-docs style: lead with real code).

Every sub-section is a worked example with three parts: a `####` heading naming the use-case, a
sentence stating the task it solves, and a code block solving it. A sub-section with no code block
is not allowed — give it a real example or fold it into a neighbour; if there is nothing to show in
code, there is no sub-section. Prefer **runnable** doctests (`cargo test --doc`) so they cannot rot;
if a realistic example needs a running app or can't compile cheaply standalone, use ` ```no_run `
(still type-checked) or, last resort, ` ```ignore ` / ` ```text `. Doctests see only the public API.

**Keep implementation out:** do not explain which internal types are created, what device/subsystem
is opened, the internal data flow, or the algorithms used (sampling, interpolation, threading, FFI,
buffering, ...). Describe the result the user cares about, not the machinery — e.g. *"this enables
audio playback in the world"*, not *"this opens the default output device and inserts the
`AudioStateResource`"*; *"`resample_audio` converts a buffer to a different sample rate"*, not
*"...with cubic interpolation"*. Such "how it works inside" detail goes in *How to maintain*.

### How to maintain

Explanatory prose (no code blocks required) on the architecture, data flow, non-obvious
implementation choices, invariants, and caveats a maintainer must know before changing the code.
This is where understanding of the **private** code lives — as prose here, never as `///` items.

### Accuracy: every word must match the code

This is the rule that gets violated most. **Describe only what the code actually does — do not
invent mechanisms, names, or behavior.**

- Trace the real control flow before writing. If you cannot point to the line that does a thing,
  do not claim it happens.
- Do not introduce vocabulary the code does not justify. A past draft wrote that a shape is
  *"re-indexed"* each frame — there is no indexing step in the code; the system simply rebuilds
  the resource from scratch. Use the term that matches what the code literally does ("rebuilt",
  "transformed", "inserted"), not an invented one.
- Prefer naming the concrete function/type that performs a step (with an intra-doc link) over a
  vague description, so the claim stays checkable.

**Intra-doc links:** link items you mention with `` [`Type`] `` / `[text](Self::method)` /
`` [`crate::Item`] `` so the reference is clickable. This is the *only* place links are written
(there are no item docs to link from).

## 4. Touch only the main page

This skill edits exactly one file: the main page that carries the `//!` overview. Every other
source file in the target scope is **read-only** — you read it to understand the code, but you
never modify it. Concretely:

- Do not remove or rewrite existing `///` item docs in other files.
- Do not delete, move, or reformat developer comments (`//`, `// todo:`, block comments) anywhere.
- Do not make any other incidental edit (imports, formatting, renames) outside the overview.

If another file holds knowledge worth surfacing — a non-obvious invariant in a `///` doc, a
`// todo:`, or a non-Rust doc style such as C#/XML `/// <summary>...</summary>` — capture that
knowledge as prose in the *How to maintain* section of the overview. Leave the original comment
exactly where it is; never paraphrase it away in its own file.

## 5. Verify

From the repo root, in PowerShell:

```powershell
$env:RUSTDOCFLAGS = "-D rustdoc::broken-intra-doc-links"
cargo +nightly doc -p <owning_crate> --no-deps
```

- Fix any errors or broken-link warnings and re-run until clean.
- If the overview contains runnable examples, also run `cargo +nightly test --doc -p <owning_crate>`.

## 6. Report

Summarize:
- the crate/module overview you wrote (its sub-sections and examples),
- any non-obvious knowledge you carried from other files into *How to maintain* (their source
  comments stay in place),
- confirmation that no file other than the main page was modified,
- the verification result (doc build / doc-test status).

Do **not** commit — only when the user explicitly asks.
