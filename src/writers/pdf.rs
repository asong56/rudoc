//! PDF writer with two strategies:
//!  1. Subprocess: call `typst` CLI if available on PATH (best quality)
//!  2. Built-in:   `printpdf` with embedded Helvetica (feature = "pdf")
//!  3. Fallback:   helpful error with instructions

use anyhow::{bail, Result};
use crate::ir::doc::DocIR;
use crate::writers::typst_writer;

pub fn render(doc: &DocIR, paper: &str, font: &str) -> Result<Vec<u8>> {
    let typ_source = typst_writer::render(doc, paper, font);

    // Strategy 1: typst CLI subprocess (zero extra binary size)
    if let Some(bytes) = try_typst_subprocess(&typ_source) {
        return Ok(bytes);
    }

    // Strategy 2: built-in printpdf (requires feature = "pdf")
    #[cfg(feature = "pdf")]
    {
        return render_printpdf(doc, paper);
    }

    // Strategy 3: helpful error
    #[cfg(not(feature = "pdf"))]
    bail!(
        "PDF output requires either:\n\
         • Install typst on your PATH: https://typst.app (recommended, best quality)\n\
         • Rebuild rudoc with PDF built-in: cargo build --features pdf\n\
         • Convert to .typ first then run typst manually: rudoc input.md output.typ"
    )
}

/// Try to call the `typst` CLI using properly unique temp file paths.
fn try_typst_subprocess(typ_source: &str) -> Option<Vec<u8>> {
    // Use a thread-local counter + pid for guaranteed-unique names
    // without any external dependency.
    let id = unique_id();
    let tmp_dir  = std::env::temp_dir();
    let typ_path = tmp_dir.join(format!("rudoc_{}_{}.typ", std::process::id(), id));
    let pdf_path = tmp_dir.join(format!("rudoc_{}_{}.pdf", std::process::id(), id));

    std::fs::write(&typ_path, typ_source).ok()?;

    let status = std::process::Command::new("typst")
        .args(["compile", typ_path.to_str()?, pdf_path.to_str()?])
        .stderr(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .status()
        .ok()?;

    let _ = std::fs::remove_file(&typ_path);

    if !status.success() {
        let _ = std::fs::remove_file(&pdf_path);
        return None;
    }

    let bytes = std::fs::read(&pdf_path).ok()?;
    let _ = std::fs::remove_file(&pdf_path);
    Some(bytes)
}

/// Monotonically increasing counter, unique within this process.
fn unique_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static CTR: AtomicU64 = AtomicU64::new(0);
    CTR.fetch_add(1, Ordering::Relaxed)
}

/// Pure-Rust PDF using printpdf with Helvetica built-in font.
#[cfg(feature = "pdf")]
fn render_printpdf(doc: &DocIR, paper: &str) -> Result<Vec<u8>> {
    use printpdf::*;

    let (width_mm, height_mm): (f64, f64) = match paper {
        "a3"        => (297.0, 420.0),
        "a5"        => (148.0, 210.0),
        "us-letter" => (215.9, 279.4),
        "us-legal"  => (215.9, 355.6),
        _           => (210.0, 297.0), // a4 default
    };

    let (doc_pdf, page1, layer1) = PdfDocument::new(
        doc.metadata.title.as_deref().unwrap_or("Document"),
        Mm(width_mm),
        Mm(height_mm),
        "Layer 1",
    );

    let font      = doc_pdf.add_builtin_font(BuiltinFont::Helvetica)?;
    let font_bold = doc_pdf.add_builtin_font(BuiltinFont::HelveticaBold)?;

    let margin_mm   = 20.0_f64;
    let line_height = 6.0_f64;
    let text_width  = width_mm - 2.0 * margin_mm;
    let mut cursor_y = height_mm - margin_mm;

    let mut pages: Vec<(PdfPageIndex, PdfLayerIndex)> = vec![(page1, layer1)];
    let mut page_idx = 0usize;

    macro_rules! ensure_space {
        ($needed:expr) => {
            if cursor_y < margin_mm + $needed {
                let (np, nl) = doc_pdf.add_page(Mm(width_mm), Mm(height_mm), "Layer 1");
                pages.push((np, nl));
                page_idx += 1;
                cursor_y = height_mm - margin_mm;
            }
        };
    }

    macro_rules! layer {
        () => { doc_pdf.get_page(pages[page_idx].0).get_layer(pages[page_idx].1) };
    }

    for block in &doc.blocks {
        match block {
            crate::ir::doc::Block::Heading(level, inlines) => {
                let size = match level { 1 => 18.0, 2 => 14.0, 3 => 12.0, _ => 11.0 };
                cursor_y -= if *level <= 2 { line_height } else { line_height * 0.5 };
                ensure_space!(size * 0.5 + line_height);
                let mut text = String::new();
                for il in inlines { crate::ir::doc::inline_to_text(il, &mut text); }
                layer!().use_text(&text, size, Mm(margin_mm), Mm(cursor_y), &font_bold);
                cursor_y -= line_height * (size / 11.0);
            }
            crate::ir::doc::Block::Para(inlines) => {
                let mut text = String::new();
                for il in inlines { crate::ir::doc::inline_to_text(il, &mut text); }
                let chars_per_line = (text_width / 2.2) as usize;
                for line in &word_wrap(&text, chars_per_line) {
                    ensure_space!(line_height * 2.0);
                    layer!().use_text(line, 11.0, Mm(margin_mm), Mm(cursor_y), &font);
                    cursor_y -= line_height;
                }
                cursor_y -= line_height * 0.5;
            }
            crate::ir::doc::Block::CodeBlock { code, .. } => {
                let font_mono = doc_pdf.add_builtin_font(BuiltinFont::Courier)?;
                for line in code.lines() {
                    ensure_space!(line_height);
                    layer!().use_text(line, 9.0, Mm(margin_mm + 4.0), Mm(cursor_y), &font_mono);
                    cursor_y -= line_height * 0.9;
                }
                cursor_y -= line_height * 0.5;
            }
            crate::ir::doc::Block::List { ordered, start, items, .. } => {
                for (i, item) in items.iter().enumerate() {
                    let bullet = if *ordered { format!("{}.", i as u64 + start) } else { "•".to_string() };
                    let text = item.iter().find_map(|b| {
                        if let crate::ir::doc::Block::Para(ils) = b {
                            let mut s = String::new();
                            for il in ils { crate::ir::doc::inline_to_text(il, &mut s); }
                            Some(s)
                        } else { None }
                    }).unwrap_or_default();
                    ensure_space!(line_height);
                    layer!().use_text(&bullet, 11.0, Mm(margin_mm),       Mm(cursor_y), &font);
                    layer!().use_text(&text,   11.0, Mm(margin_mm + 6.0), Mm(cursor_y), &font);
                    cursor_y -= line_height;
                }
                cursor_y -= line_height * 0.5;
            }
            crate::ir::doc::Block::HorizontalRule => {
                cursor_y -= line_height;
                ensure_space!(line_height);
                let line = Line {
                    points: vec![
                        (Point::new(Mm(margin_mm), Mm(cursor_y + line_height / 2.0)), false),
                        (Point::new(Mm(width_mm - margin_mm), Mm(cursor_y + line_height / 2.0)), false),
                    ],
                    is_closed: false, has_fill: false, has_stroke: true, is_clipping_path: false,
                };
                layer!().add_shape(line);
                cursor_y -= line_height;
            }
            other => {
                let mut text = String::new();
                crate::ir::doc::block_to_text_pub(other, &mut text);
                for line in text.lines() {
                    ensure_space!(line_height);
                    layer!().use_text(line, 11.0, Mm(margin_mm), Mm(cursor_y), &font);
                    cursor_y -= line_height;
                }
            }
        }
    }

    let mut buf = Vec::new();
    doc_pdf.save(&mut std::io::BufWriter::new(&mut buf))?;
    Ok(buf)
}

fn word_wrap(text: &str, chars_per_line: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut current = String::new();
    for word in words {
        if current.len() + word.len() + 1 > chars_per_line && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() { current.push(' '); }
        current.push_str(word);
    }
    if !current.is_empty() { lines.push(current); }
    if lines.is_empty() { lines.push(String::new()); }
    lines
}
