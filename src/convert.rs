use anyhow::{bail, Result};

use crate::detect::{Format, IrTier};
use crate::error::RudocError;
use crate::ir::doc::DocIR;
use crate::ir::slide::SlideIR;
use crate::{readers, writers};

/// Validates `input` as UTF-8 for a text-based `format`, producing a
/// `RudocError::InvalidUtf8` (exit code 65/DATA_ERR) instead of a generic
/// error on failure — this one helper covers every text-format reader
/// below instead of each repeating its own ad-hoc `std::str::from_utf8`.
fn require_utf8(input: &[u8], format: Format) -> Result<&str> {
    std::str::from_utf8(input).map_err(|_| RudocError::InvalidUtf8 { format }.into())
}

/// All user-facing options that affect conversion.
#[derive(Debug, Clone)]
pub struct ConvertOptions {
    pub from: Format,
    pub to: Format,
    pub standalone: bool,
    pub slide_level: u8,
    pub sheet_name: String,
    pub pdf_paper: String,
    pub pdf_font: String,
    pub wrap: Option<usize>,
    pub verbose: bool,
    pub quiet: bool,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        ConvertOptions {
            from: Format::Markdown,
            to: Format::Html,
            standalone: false,
            slide_level: 1,
            sheet_name: "Sheet1".to_string(),
            pdf_paper: "a4".to_string(),
            pdf_font: "Arial".to_string(),
            wrap: None,
            verbose: false,
            quiet: false,
        }
    }
}

/// Input to the converter — either raw bytes (single file / binary formats)
/// or a pre-parsed and merged DocIR (multi-file doc-tier path).
pub enum Input {
    Bytes(Vec<u8>),
    Doc(DocIR),
}

/// Convert input to output bytes.
pub fn convert(input: Input, opts: &ConvertOptions) -> Result<Vec<u8>> {
    let from = opts.from;
    let to = opts.to;

    if opts.verbose {
        eprintln!("[rudoc] {} → {}", from, to);
    }

    // ── md → pptx (special cross-tier path) ─────────────────────────
    if from == Format::Markdown && to == Format::Pptx {
        let doc = input_to_doc(input, opts)?;
        let slides = SlideIR::from_doc(&doc, opts.slide_level);
        if opts.verbose { eprintln!("[rudoc] {} slides generated", slides.slides.len()); }
        return writers::pptx::render(&slides);
    }

    // Validate tier compatibility
    let from_tier = from.ir_tier();
    let to_tier = to.ir_tier();
    if from_tier != to_tier {
        return Err(RudocError::IncompatibleFormats { from, to }.into());
    }

    match from_tier {
        IrTier::Doc => {
            let doc = input_to_doc(input, opts)?;
            convert_doc_ir(doc, to, opts)
        }
        IrTier::Table => {
            let bytes = input_to_bytes(input);
            convert_table(&bytes, from, to, opts)
        }
        IrTier::Tree => {
            let bytes = input_to_bytes(input);
            convert_tree(&bytes, from, to, opts)
        }
        IrTier::Slide => return Err(RudocError::UnsupportedConversion { from, to }.into()),
    }
}

fn input_to_bytes(input: Input) -> Vec<u8> {
    match input {
        Input::Bytes(b) => b,
        Input::Doc(_) => unreachable!("Doc input should not reach table/tree tier"),
    }
}

fn input_to_doc(input: Input, opts: &ConvertOptions) -> Result<DocIR> {
    match input {
        Input::Doc(doc) => Ok(doc),
        Input::Bytes(bytes) => parse_doc(&bytes, opts.from),
    }
}

fn parse_doc(input: &[u8], from: Format) -> Result<DocIR> {
    match from {
        Format::Markdown => {
            let src = require_utf8(input, from)?;
            let mut doc = readers::markdown::parse(src)?;
            if doc.metadata.title.is_none() {
                doc.metadata.title = readers::markdown::extract_title(src);
            }
            Ok(doc)
        }
        Format::Html => {
            let src = require_utf8(input, from)?;
            readers::html::parse(src)
        }
        Format::Txt => {
            let src = require_utf8(input, from)?;
            readers::txt::parse(src)
        }
        Format::Docx => readers::docx::parse(input),
        Format::Typst => {
            let src = require_utf8(input, from)?;
            readers::typst_reader::parse(src)
        }
        Format::Pdf => Err(RudocError::PdfReadNotImplemented.into()),
        _ => bail!("Unexpected format in doc tier: {}", from),
    }
}

// ── Doc tier ─────────────────────────────────────────────────────────────

fn convert_doc_ir(doc: DocIR, to: Format, opts: &ConvertOptions) -> Result<Vec<u8>> {
    if opts.verbose {
        eprintln!("[rudoc] DocIR: {} blocks", doc.blocks.len());
    }

    match to {
        Format::Markdown => {
            let s = writers::markdown::render(&doc, opts.wrap);
            Ok(s.into_bytes())
        }
        Format::Html => {
            let s = writers::html::render(&doc, opts.standalone);
            Ok(s.into_bytes())
        }
        Format::Txt => {
            let s = writers::txt::render(&doc, opts.wrap);
            Ok(s.into_bytes())
        }
        Format::Docx => writers::docx::render(&doc),
        Format::Typst => {
            let s = writers::typst_writer::render(&doc, &opts.pdf_paper, &opts.pdf_font);
            Ok(s.into_bytes())
        }
        Format::Pdf => writers::pdf::render(&doc, &opts.pdf_paper, &opts.pdf_font),
        _ => bail!("Unexpected format in doc tier: {}", to),
    }
}

// ── Table tier ────────────────────────────────────────────────────────────

fn convert_table(input: &[u8], from: Format, to: Format, opts: &ConvertOptions) -> Result<Vec<u8>> {
    let table = match from {
        Format::Csv => {
            let src = require_utf8(input, from)?;
            readers::csv::parse(src, &opts.sheet_name)?
        }
        Format::Xlsx => readers::xlsx::parse(input, None)?,
        _ => bail!("Unexpected format in table tier: {}", from),
    };

    if opts.verbose {
        eprintln!("[rudoc] TableIR: {} sheets, {} rows (first sheet)",
            table.sheets.len(),
            table.sheets.first().map(|s| s.rows.len()).unwrap_or(0));
    }

    match to {
        Format::Csv => {
            let s = writers::csv::render(&table)?;
            Ok(s.into_bytes())
        }
        Format::Xlsx => writers::xlsx::render(&table),
        _ => bail!("Unexpected format in table tier: {}", to),
    }
}

// ── Tree tier ─────────────────────────────────────────────────────────────

fn convert_tree(input: &[u8], from: Format, to: Format, opts: &ConvertOptions) -> Result<Vec<u8>> {
    let src = require_utf8(input, from)?;
    let tree = match from {
        Format::Xml  => readers::xml::parse(src)?,
        Format::Opml => readers::opml::parse(src)?,
        Format::Json => readers::json::parse(src)?,
        _ => bail!("Unexpected format in tree tier: {}", from),
    };

    if opts.verbose {
        eprintln!("[rudoc] TreeIR root: <{}>  {} children", tree.tag, tree.children.len());
    }

    match to {
        Format::Xml => {
            let s = writers::xml::render(&tree, true)?;
            Ok(s.into_bytes())
        }
        Format::Opml => {
            let s = writers::opml::render(&tree)?;
            Ok(s.into_bytes())
        }
        Format::Json => {
            let s = writers::json::render(&tree, true)?;
            Ok(s.into_bytes())
        }
        _ => bail!("Unexpected format in tree tier: {}", to),
    }
}

/// Merge multiple DocIR inputs into one, separated by horizontal rules.
pub fn merge_docs(docs: Vec<DocIR>) -> DocIR {
    use crate::ir::doc::Block;
    let mut merged = DocIR::new();
    if let Some(first) = docs.first() {
        merged.metadata = first.metadata.clone();
    }
    for (i, mut doc) in docs.into_iter().enumerate() {
        if i > 0 {
            merged.blocks.push(Block::HorizontalRule);
        }
        merged.blocks.append(&mut doc.blocks);
    }
    merged
}
