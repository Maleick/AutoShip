Third-party reference code lives here.

`third_party/eqlib` and `third_party/macroquest` are git submodules that track the upstream
MacroQuest repositories.

Fresh clones can fetch them up front with:

`git clone --recurse-submodules https://github.com/Maleick/DMFT.git`

Existing clones can initialize or update them with:

`git submodule update --init --recursive`

Use `--recursive` because `third_party/macroquest` contains its own upstream submodules
such as `src/eqlib`, `contrib/vcpkg`, and Lua integration tests.

Routine `cargo build` / `cargo test` work does not require these reference trees, but
offset/struct work and submodule checkout validation do.

The top-level `third_party/eqlib` directory is the canonical eqlib tree for this
repository and should be used when adding or citing offsets, struct definitions,
or other eqlib headers. The `third_party/macroquest/src/eqlib` tree is kept only
as part of the upstream MacroQuest submodule; treat it as vendor code rather than
the primary source for DMFT references.
