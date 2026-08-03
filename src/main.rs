#![allow(dead_code, unused_variables)]
mod cli;
mod convert;
mod detect;
mod error;
mod ir;
mod readers;
mod writers;

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{bail, Context, Result};
use clap::Parser;
use serde_json::json;

use cli::Cli;
use convert::{convert, merge_docs, ConvertOptions};
use detect::Format;
use error::RudocError;

fn main() {
    let cli = Cli::parse();
    let json_mode = cli.json;
    if let Err(e) = run(cli) {
        report_error(&e, json_mode);
        // `downcast_ref` finds a `RudocError` even through `.context()`
        // wrapping — anyhow's context chain recurses to the root cause —
        // so this reliably picks the specific code when one was set.
        let code = e
            .downcast_ref::<RudocError>()
            .map(RudocError::exit_code)
            .unwrap_or(error::exit_code::SOFTWARE);
        std::process::exit(code);
    }
}

/// Prints one failure, either as a human-readable line or as a
/// `{"event":"error",...}` JSON Lines record — see `Cli::json`'s doc
/// comment for the stable field contract external tools can rely on.
fn report_error(e: &anyhow::Error, json_mode: bool) {
    if json_mode {
        let (code, exit_code) = match e.downcast_ref::<RudocError>() {
            Some(re) => (re.code_name(), re.exit_code()),
            None => ("internal_error", error::exit_code::SOFTWARE),
        };
        eprintln!(
            "{}",
            json!({
                "event": "error",
                "code": code,
                "exit_code": exit_code,
                "message": format!("{:#}", e),
            })
        );
    } else {
        eprintln!("error: {:#}", e);
    }
}

fn run(cli: Cli) -> Result<()> {
    let start = Instant::now();
    let json_mode = cli.json;

    let (input_paths, output_path) = resolve_paths(&cli)?;
    let from_fmt = detect_from(&cli, &input_paths)?;
    let to_fmt = detect_to(&cli, &output_path, from_fmt)?;

    let opts = ConvertOptions {
        from: from_fmt,
        to: to_fmt,
        standalone: cli.standalone
            || (to_fmt == Format::Html && output_path.as_deref() != Some("-")),
        slide_level: cli.slide_level,
        sheet_name: cli.sheet.clone(),
        pdf_paper: cli.pdf_paper.clone(),
        pdf_font: cli.pdf_font.clone(),
        wrap: cli.wrap,
        verbose: cli.verbose,
        quiet: cli.quiet,
    };

    let in_desc = input_paths.first().map(|s| s.as_str()).unwrap_or("-");
    let out_desc = output_path.as_deref().unwrap_or("-");

    if json_mode {
        eprintln!(
            "{}",
            json!({
                "event": "start",
                "from": from_fmt.to_string(),
                "to": to_fmt.to_string(),
                "input": in_desc,
                "output": out_desc,
            })
        );
    } else if !cli.quiet {
        eprintln!("[rudoc] {} → {}  ({}→{})", in_desc, out_desc, from_fmt, to_fmt);
    }

    let merged_doc = read_inputs(&input_paths, &opts)?;
    let output_bytes = convert(merged_doc, &opts)
        .with_context(|| format!("Conversion failed ({} → {})", from_fmt, to_fmt))?;

    write_output(&output_bytes, output_path.as_deref())?;

    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    if json_mode {
        eprintln!(
            "{}",
            json!({
                "event": "done",
                "bytes": output_bytes.len(),
                "ms": elapsed_ms,
            })
        );
    } else if !cli.quiet {
        eprintln!("[rudoc] done in {:.0}ms  ({} bytes)", elapsed_ms, output_bytes.len());
    }
    Ok(())
}

fn resolve_paths(cli: &Cli) -> Result<(Vec<String>, Option<String>)> {
    if cli.output.is_some() {
        return Ok((cli.inputs.clone(), cli.output.clone()));
    }
    let mut inputs = cli.inputs.clone();
    if inputs.is_empty() {
        return Ok((vec!["-".to_string()], None));
    }
    if cli.to.is_some() {
        return Ok((inputs, None));
    }
    if inputs.len() >= 2 {
        let last = inputs.pop().unwrap();
        return Ok((inputs, Some(last)));
    }
    Ok((inputs, None))
}

fn detect_from(cli: &Cli, input_paths: &[String]) -> Result<Format> {
    if let Some(ref s) = cli.from {
        return Format::from_name(s);
    }
    let first = input_paths.first().map(|s| s.as_str()).unwrap_or("-");
    if first == "-" {
        return Err(RudocError::UnknownStdinFormat.into());
    }
    Format::from_path(Path::new(first))
}

fn detect_to(cli: &Cli, output: &Option<String>, from_fmt: Format) -> Result<Format> {
    if let Some(ref s) = cli.to {
        return Format::from_name(s);
    }
    match output.as_deref() {
        None | Some("-") => Err(RudocError::UnknownStdoutFormat { from: from_fmt }.into()),
        Some(p) => Format::from_path(Path::new(p)),
    }
}

/// Reads and merges inputs. Returns a merged DocIR for doc-tier formats,
/// or raw bytes for everything else (single file only).
fn read_inputs(paths: &[String], opts: &ConvertOptions) -> Result<convert::Input> {
    use detect::IrTier;

    if paths.len() == 1 {
        let bytes = read_one(&paths[0])?;
        return Ok(convert::Input::Bytes(bytes));
    }

    // Multi-file input is only meaningful for doc-tier text formats (they
    // can be concatenated block-by-block); other tiers have no defined
    // merge semantics, so this stays a plain, uncoded usage error.
    if opts.from.ir_tier() != IrTier::Doc {
        bail!("Multiple input files are only supported for document formats.");
    }

    let mut docs = Vec::new();
    for path in paths {
        let bytes = read_one(path)?;
        let src = std::str::from_utf8(&bytes)
            .map_err(|_| RudocError::InvalidUtf8 { format: opts.from })?;
        let doc = match opts.from {
            Format::Markdown => readers::markdown::parse(src)?,
            Format::Html => readers::html::parse(src)?,
            Format::Txt => readers::txt::parse(src)?,
            Format::Typst => readers::typst_reader::parse(src)?,
            _ => {
                if !opts.quiet {
                    eprintln!("[rudoc] warning: only first file used for binary format {}", opts.from);
                }
                let bytes = read_one(&paths[0])?;
                return Ok(convert::Input::Bytes(bytes));
            }
        };
        if opts.verbose {
            eprintln!("[rudoc] parsed {} blocks from {}", doc.blocks.len(), path);
        }
        docs.push(doc);
    }

    let merged = merge_docs(docs);
    if opts.verbose {
        eprintln!("[rudoc] merged {} blocks total", merged.blocks.len());
    }
    Ok(convert::Input::Doc(merged))
}

fn read_one(path: &str) -> Result<Vec<u8>> {
    if path == "-" {
        let mut buf = Vec::new();
        io::stdin().read_to_end(&mut buf)?;
        return Ok(buf);
    }
    std::fs::read(path).map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            RudocError::InputNotFound(PathBuf::from(path)).into()
        } else {
            RudocError::ReadFailed { path: PathBuf::from(path), source: e }.into()
        }
    })
}

fn write_output(bytes: &[u8], path: Option<&str>) -> Result<()> {
    match path {
        None | Some("-") => {
            io::stdout().write_all(bytes)?;
        }
        Some(p) => {
            if let Some(parent) = Path::new(p).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).map_err(|e| RudocError::WriteFailed {
                        path: PathBuf::from(p),
                        source: e,
                    })?;
                }
            }
            std::fs::write(p, bytes).map_err(|e| RudocError::WriteFailed {
                path: PathBuf::from(p),
                source: e,
            })?;
        }
    }
    Ok(())
}
