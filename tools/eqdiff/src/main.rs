use anyhow::{Context, Result, bail};
use eqdiff::{
    apply_matches_to_offsets_json, extract_strings, match_functions_by_string_references,
    match_string_references, parse_pe, string_function_refs_from_xrefs,
};
use serde_json::Value;
use std::{env, ffi::OsString, fs, path::PathBuf};

fn main() -> Result<()> {
    let args = Args::parse(env::args_os().skip(1))?;

    let old_bytes = fs::read(&args.old_binary)
        .with_context(|| format!("failed to read {}", args.old_binary.display()))?;
    let new_bytes = fs::read(&args.new_binary)
        .with_context(|| format!("failed to read {}", args.new_binary.display()))?;
    let offsets_text = fs::read_to_string(&args.offsets)
        .with_context(|| format!("failed to read {}", args.offsets.display()))?;
    let offsets: Value = serde_json::from_str(&offsets_text)
        .with_context(|| format!("failed to parse {}", args.offsets.display()))?;

    let old_pe = parse_pe(&old_bytes)?;
    let new_pe = parse_pe(&new_bytes)?;

    let old_strings = extract_strings(&old_pe, &old_bytes);
    let new_strings = extract_strings(&new_pe, &new_bytes);
    let old_xrefs = match_string_references(&old_pe, &old_bytes, old_strings);
    let new_xrefs = match_string_references(&new_pe, &new_bytes, new_strings);
    let old_refs = string_function_refs_from_xrefs(&old_xrefs);
    let new_refs = string_function_refs_from_xrefs(&new_xrefs);
    let matches = match_functions_by_string_references(&old_refs, &new_refs);

    let old_base = old_pe.image_base as u64;
    let new_base = new_pe.image_base as u64;
    let updated_offsets =
        apply_matches_to_offsets_json(offsets.clone(), old_base, new_base, &matches)?;

    let output_path = args
        .output
        .unwrap_or_else(|| args.offsets.with_extension("updated.json"));
    let report_path = args
        .report
        .unwrap_or_else(|| args.offsets.with_extension("eqdiff-report.md"));

    fs::write(
        &output_path,
        serde_json::to_string_pretty(&updated_offsets)?,
    )
    .with_context(|| format!("failed to write {}", output_path.display()))?;
    fs::write(
        &report_path,
        build_report(old_base, new_base, &offsets, &matches),
    )
    .with_context(|| format!("failed to write {}", report_path.display()))?;

    println!(
        "matched {} functions; wrote {} and {}",
        matches.len(),
        output_path.display(),
        report_path.display()
    );
    Ok(())
}

struct Args {
    old_binary: PathBuf,
    new_binary: PathBuf,
    offsets: PathBuf,
    output: Option<PathBuf>,
    report: Option<PathBuf>,
}

impl Args {
    fn parse(raw_args: impl Iterator<Item = OsString>) -> Result<Self> {
        let mut positional = Vec::new();
        let mut offsets = None;
        let mut output = None;
        let mut report = None;
        let mut args = raw_args.peekable();

        while let Some(arg) = args.next() {
            match arg.to_string_lossy().as_ref() {
                "--offsets" => offsets = Some(next_path(&mut args, "--offsets")?),
                "--output" => output = Some(next_path(&mut args, "--output")?),
                "--report" => report = Some(next_path(&mut args, "--report")?),
                "--help" | "-h" => bail!(usage()),
                flag if flag.starts_with('-') => bail!("unknown flag {flag}\n{}", usage()),
                _ => positional.push(PathBuf::from(arg)),
            }
        }

        if positional.len() != 2 {
            bail!(usage());
        }

        Ok(Self {
            old_binary: positional.remove(0),
            new_binary: positional.remove(0),
            offsets: offsets.context("missing required --offsets <path>")?,
            output,
            report,
        })
    }
}

fn next_path(args: &mut impl Iterator<Item = OsString>, flag: &str) -> Result<PathBuf> {
    args.next()
        .map(PathBuf::from)
        .with_context(|| format!("missing value for {flag}"))
}

fn usage() -> &'static str {
    "usage: eqdiff <old-eqgame.exe> <new-eqgame.exe> --offsets <offsets.json> [--output <path>] [--report <path>]"
}

fn build_report(
    old_base: u64,
    new_base: u64,
    offsets: &Value,
    matches: &[eqdiff::FunctionMatch],
) -> String {
    let functions = offsets
        .get("functions")
        .and_then(Value::as_object)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|(name, value)| value.as_u64().map(|addr| (addr, name.as_str())))
                .collect::<std::collections::HashMap<_, _>>()
        })
        .unwrap_or_default();

    let mut report = String::new();
    report.push_str("# EQDiff Report\n\n");
    report.push_str(&format!("Old preferred base: `{old_base:#x}`\n\n"));
    report.push_str(&format!("New preferred base: `{new_base:#x}`\n\n"));
    report.push_str(&format!("Matched functions: `{}`\n\n", matches.len()));
    report.push_str("| Name | Old address | New address | Confidence | Evidence |\n");
    report.push_str("| --- | ---: | ---: | ---: | --- |\n");

    for entry in matches {
        let old_addr = old_base + u64::from(entry.old_rva);
        let new_addr = new_base + u64::from(entry.new_rva);
        let name = functions.get(&old_addr).copied().unwrap_or("(untracked)");
        let evidence = entry.matched_strings.join("<br>");
        report.push_str(&format!(
            "| `{name}` | `{old_addr:#x}` | `{new_addr:#x}` | `{:.2}` | {} |\n",
            entry.confidence, evidence
        ));
    }

    report
}
