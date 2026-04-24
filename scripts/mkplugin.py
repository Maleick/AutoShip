#!/usr/bin/env python3
"""Create a minimal TextQuest plugin crate."""

from __future__ import annotations

import argparse
from pathlib import Path


def rust_type_name(name: str) -> str:
    return "".join(part.capitalize() for part in name.replace("_", "-").split("-")) + "Plugin"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("name", help="crate name, for example my-inventory-plugin")
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=None,
        help="plugin crate output directory; defaults to plugins/<name>",
    )
    parser.add_argument(
        "--common-path",
        default="../../textquest-common",
        help="relative path from the plugin crate to textquest-common",
    )
    args = parser.parse_args()

    crate_dir = args.out_dir or Path("plugins") / args.name
    src_dir = crate_dir / "src"
    cargo_toml = crate_dir / "Cargo.toml"
    lib_rs = src_dir / "lib.rs"

    if crate_dir.exists():
        raise SystemExit(f"refusing to overwrite existing directory: {crate_dir}")

    type_name = rust_type_name(args.name)
    src_dir.mkdir(parents=True)

    cargo_toml.write_text(
        f"""[package]
name = "{args.name}"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
textquest-common = {{ path = "{args.common_path}" }}
""",
        encoding="utf-8",
    )

    lib_rs.write_text(
        f"""use textquest_common::plugins::{{
    PluginCapability, PluginContext, PluginDomain, PluginMetadata, PluginResult, TextQuestPlugin,
}};

pub struct {type_name};

impl TextQuestPlugin for {type_name} {{
    fn metadata(&self) -> PluginMetadata {{
        PluginMetadata::new("{args.name}", env!("CARGO_PKG_VERSION"), "TextQuest plugin")
    }}

    fn on_load(&mut self, context: &mut PluginContext) -> PluginResult<()> {{
        context.register_capability(PluginCapability::new(
            PluginDomain::System,
            "{args.name}.loaded",
            "Registers a system extension point.",
        ));
        Ok(())
    }}
}}

pub fn plugin() -> Box<dyn TextQuestPlugin> {{
    Box::new({type_name})
}}
""",
        encoding="utf-8",
    )

    print(f"created {crate_dir}")
    print("add the crate to Cargo.toml workspace.members when it should build in-tree")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
