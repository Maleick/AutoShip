Third-party reference code lives here when it is present in your local workspace.

Routine `cargo build` / `cargo test` work does not require these reference trees, but
offset, struct, login, and upstream behavior research may use them as optional local
reference material.

The top-level `third_party/eqlib` directory remains the canonical eqlib tree for this
repository and should be used when adding or citing offsets, struct definitions,
or other eqlib headers. Treat `third_party/macroquest/src/eqlib` as upstream-vendored
context rather than the primary source for TextQuest references.
