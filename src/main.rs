mod cli;
mod convert;
mod detect;
mod error;
mod ir;
mod readers;
mod writers;

use std::io::{self, Read, Write};
use std::path::Path;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use clap::Parser;

use cli::Cli;
use convert::{ConvertOptions, convert, merge_docs};
use detect::Format;

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {:#}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let start = Instant::now();

    let (input_paths, output_path) = resolve_paths(&cli)?;
    let from_fmt = detect_from(&cli, &input_paths)?;
    let to_fmt   = detect_to(&cli, &output_path)?;

    let opts = ConvertOptions {
        from: from_fmt,
        to:   to_fmt,
        standalone:  cli.standalone || (to_fmt == Format::Html && output_path.as_deref() != Some("-")),
        slide_level: cli.slide_level,
        sheet_name:  cli.sheet.clone(),
        pdf_paper:   cli.pdf_paper.clone(),
        pdf_font:    cli.pdf_font.clone(),
        wrap:        cli.wrap,
        verbose:     cli.verbose,
        quiet:       cli.quiet,
    };

    if !cli.quiet {
        let out_desc = output_path.as_deref().unwrap_or("<stdout>");
        let in_desc  = input_paths.first().map(|s| s.as_str()).unwrap_or("<stdin>");
        eprintln!("[rudoc] {} → {}  ({}→{})", in_desc, out_desc, from_fmt, to_fmt);
    }

    let input_bytes  = read_inputs(&input_paths, &opts)?;
    let output_bytes = convert(&input_bytes, &opts)
        .with_context(|| format!("Conversion failed ({} → {})", from_fmt, to_fmt))?;

    write_output(&output_bytes, output_path.as_deref())?;

    if !cli.quiet {
        eprintln!("[rudoc] done in {:.0}ms  ({} bytes)",
            start.elapsed().as_secs_f64() * 1000.0,
            output_bytes.len());
    }
    Ok(())
}

fn resolve_paths(cli: &Cli) -> Result<(Vec<String>, Option<String>)> {
    // Explicit -o always wins
    if cli.output.is_some() {
        return Ok((cli.inputs.clone(), cli.output.clone()));
    }
    let mut inputs = cli.inputs.clone();
    if inputs.is_empty() {
        return Ok((vec!["-".to_string()], None));
    }
    // If -t is given, output must be via -o; all positionals are inputs
    if cli.to.is_some() {
        return Ok((inputs, None));
    }
    // No -t: last positional is the output path (classic: rudoc in.md out.docx)
    if inputs.len() >= 2 {
        let last = inputs.pop().unwrap();
        return Ok((inputs, Some(last)));
    }
    // Single positional, no -t, no -o: can't determine output
    Ok((inputs, None))
}

fn detect_from(cli: &Cli, input_paths: &[String]) -> Result<Format> {
    if let Some(ref s) = cli.from { return Format::from_name(s); }
    let first = input_paths.first().map(|s| s.as_str()).unwrap_or("-");
    if first == "-" { bail!("Cannot detect input format from stdin. Use -f FORMAT."); }
    Format::from_path(Path::new(first))
}

fn detect_to(cli: &Cli, output: &Option<String>) -> Result<Format> {
    if let Some(ref s) = cli.to { return Format::from_name(s); }
    match output.as_deref() {
        None | Some("-") => bail!("Cannot detect output format. Use -t FORMAT or specify an output file."),
        Some(p) => Format::from_path(Path::new(p)),
    }
}

fn read_inputs(paths: &[String], opts: &ConvertOptions) -> Result<Vec<u8>> {
    if paths.len() == 1 {
        return read_one(&paths[0]);
    }
    use detect::IrTier;
    if opts.from.ir_tier() != IrTier::Doc {
        bail!("Multiple input files are only supported for document formats.");
    }
    let mut docs = Vec::new();
    for path in paths {
        let bytes = read_one(path)?;
        eprintln!("[rudoc-debug] bytes len={} first20={:?}", bytes.len(), &bytes[..bytes.len().min(20)]);
        let src = std::str::from_utf8(&bytes)
            .with_context(|| format!("'{}' is not valid UTF-8", path))?;
        let doc = match opts.from {
            Format::Markdown => readers::markdown::parse(src)?,
            Format::Html     => readers::html::parse(src)?,
            Format::Txt      => readers::txt::parse(src)?,
            Format::Typst    => readers::typst_reader::parse(src)?,
            _ => {
                eprintln!("[rudoc] warning: only first file used for binary format {}", opts.from);
                return read_one(&paths[0]);
            }
        };
        eprintln!("[rudoc-debug] parsed {} blocks from {}", doc.blocks.len(), path);
        docs.push(doc);
    }
    let merged = merge_docs(docs);
    let merged_md = writers::markdown::render(&merged, false);
    eprintln!("[rudoc-debug] merged {} blocks, {} bytes of md", merged.blocks.len(), merged_md.len());
    Ok(merged_md.into_bytes())
}

fn read_one(path: &str) -> Result<Vec<u8>> {
    if path == "-" {
        let mut buf = Vec::new();
        io::stdin().read_to_end(&mut buf)?;
        Ok(buf)
    } else {
        std::fs::read(path).with_context(|| format!("Cannot read '{}'", path))
    }
}

fn write_output(bytes: &[u8], path: Option<&str>) -> Result<()> {
    match path {
        None | Some("-") => { io::stdout().write_all(bytes)?; }
        Some(p) => {
            if let Some(parent) = Path::new(p).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            std::fs::write(p, bytes)
                .with_context(|| format!("Cannot write to '{}'", p))?;
        }
    }
    Ok(())
}
