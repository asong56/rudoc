//! DOCX writer using docx-rs.
//! Writes directly to an in-memory buffer — no temp files.
use anyhow::Result;
use docx_rs::*;
use std::io::Cursor;
use std::path::Path;

use crate::ir::doc::{Block, DocIR, Inline};

pub fn render(doc: &DocIR) -> Result<Vec<u8>> {
    build_docx_bytes(doc)
}

fn build_docx_bytes(doc: &DocIR) -> Result<Vec<u8>> {
    let mut docx = Docx::new();

    // Title from metadata
    if let Some(title) = &doc.metadata.title {
        let para = Paragraph::new()
            .add_run(Run::new().add_text(title).bold())
            .style("Title");
        docx = docx.add_paragraph(para);
    }

    let mut num_id_counter = 1usize;
    for block in &doc.blocks {
        (docx, num_id_counter) = add_block(docx, block, num_id_counter)?;
    }

    // Write directly to an in-memory cursor — no temp file needed
    let mut buf: Vec<u8> = Vec::new();
    docx.build()
        .pack(Cursor::new(&mut buf))
        .map_err(|e| anyhow::anyhow!("DOCX pack: {}", e))?;
    Ok(buf)
}

fn add_block(docx: Docx, block: &Block, num_id: usize) -> Result<(Docx, usize)> {
    let mut num_id = num_id;
    let docx = match block {
        Block::Heading(level, inlines) => {
            let style = match level {
                1 => "Heading1", 2 => "Heading2", 3 => "Heading3",
                4 => "Heading4", 5 => "Heading5", _ => "Heading6",
            };
            let mut para = Paragraph::new().style(style);
            para = add_inlines_to_para(para, inlines);
            docx.add_paragraph(para)
        }
        Block::Para(inlines) => {
            let mut para = Paragraph::new();
            para = add_inlines_to_para(para, inlines);
            docx.add_paragraph(para)
        }
        Block::CodeBlock { code, .. } => {
            let mut d = docx;
            for line in code.lines() {
                let para = Paragraph::new().add_run(
                    Run::new().add_text(line).fonts(RunFonts::new().ascii("Courier New")),
                );
                d = d.add_paragraph(para);
            }
            d
        }
        Block::BlockQuote(blocks) => {
            let mut d = docx;
            for b in blocks {
                (d, num_id) = add_block(d, b, num_id)?;
            }
            d
        }
        Block::List { ordered, start, items, .. } => {
            let abstract_num = AbstractNumbering::new(num_id).add_level(Level::new(
                0,
                Start::new((*start).max(1) as usize),
                NumberFormat::new(if *ordered { "decimal" } else { "bullet" }),
                LevelText::new(if *ordered { "%1." } else { "•" }),
                LevelJc::new("left"),
            ));
            let mut d = docx
                .add_abstract_numbering(abstract_num)
                .add_numbering(Numbering::new(num_id, num_id));

            for item in items {
                let inlines = match item.first() {
                    Some(Block::Para(ils)) => ils.clone(),
                    Some(Block::Heading(_, ils)) => ils.clone(),
                    _ => vec![],
                };
                let mut para = Paragraph::new()
                    .numbering(NumberingId::new(num_id), IndentLevel::new(0));
                para = add_inlines_to_para(para, &inlines);
                d = d.add_paragraph(para);
                for sub in item.iter().skip(1) {
                    (d, num_id) = add_block(d, sub, num_id + 1)?;
                    num_id -= 1;
                }
            }
            num_id += 1;
            d
        }
        Block::Table { head, rows } => {
            let mut table = Table::new(vec![]);
            let header_cells: Vec<TableCell> = head.iter().map(|cell_inlines| {
                let text: String = cell_inlines.iter().map(|i| {
                    let mut s = String::new();
                    crate::ir::doc::inline_to_text(i, &mut s);
                    s
                }).collect();
                TableCell::new().add_paragraph(
                    Paragraph::new().add_run(Run::new().add_text(&text).bold())
                )
            }).collect();
            table = table.add_row(TableRow::new(header_cells));

            for row in rows {
                let cells: Vec<TableCell> = row.iter().map(|cell_inlines| {
                    let mut para = Paragraph::new();
                    para = add_inlines_to_para(para, cell_inlines);
                    TableCell::new().add_paragraph(para)
                }).collect();
                table = table.add_row(TableRow::new(cells));
            }
            docx.add_table(table)
        }
        Block::HorizontalRule => {
            docx.add_paragraph(
                Paragraph::new().add_run(Run::new().add_text("─".repeat(40)))
            )
        }
        Block::RawBlock { format, content } => {
            if format == "yaml" || crate::ir::doc::is_html_comment(content) {
                docx
            } else {
                docx.add_paragraph(Paragraph::new().add_run(Run::new().add_text(content)))
            }
        }
    };
    Ok((docx, num_id))
}

/// Accumulated character formatting for a run, tracked while walking nested
/// inlines (e.g. `**bold *and italic***`) so combinations are preserved
/// instead of only the outermost style surviving.
#[derive(Clone, Copy, Default)]
struct RunStyle {
    bold: bool,
    italic: bool,
    strike: bool,
    code: bool,
    superscript: bool,
    subscript: bool,
}

fn add_inlines_to_para(mut para: Paragraph, inlines: &[Inline]) -> Paragraph {
    for il in inlines {
        para = add_inline_to_para(para, il, RunStyle::default());
    }
    para
}

fn add_inline_to_para(para: Paragraph, il: &Inline, style: RunStyle) -> Paragraph {
    match il {
        Inline::Text(t) => para.add_run(styled_run(t, style)),
        Inline::Strong(inner) => add_inlines_styled(para, inner, RunStyle { bold: true, ..style }),
        Inline::Emph(inner) => add_inlines_styled(para, inner, RunStyle { italic: true, ..style }),
        Inline::Strikethrough(inner) => add_inlines_styled(para, inner, RunStyle { strike: true, ..style }),
        Inline::Superscript(inner) => {
            add_inlines_styled(para, inner, RunStyle { superscript: true, ..style })
        }
        Inline::Subscript(inner) => {
            add_inlines_styled(para, inner, RunStyle { subscript: true, ..style })
        }
        Inline::Code(s) => para.add_run(styled_run(s, RunStyle { code: true, ..style })),
        Inline::Link { url, content, .. } => {
            let mut hl = Hyperlink::new(url, HyperlinkType::External);
            for i in content {
                hl = add_run_to_hyperlink(hl, i, style);
            }
            para.add_hyperlink(hl)
        }
        Inline::Image { src, alt } => {
            let mut alt_text = String::new();
            for i in alt { crate::ir::doc::inline_to_text(i, &mut alt_text); }

            if let Some(pic_data) = try_embed_image(src) {
                let (w_emu, h_emu) = read_image_dims(src).unwrap_or((2743200, 1828800));
                let pic = Pic::new(&pic_data).size(w_emu, h_emu);
                para.add_run(Run::new().add_image(pic))
            } else {
                para.add_run(Run::new().add_text(format!("[Image: {}]", alt_text)))
            }
        }
        Inline::LineBreak => para.add_run(Run::new().add_break(BreakType::TextWrapping)),
        Inline::SoftBreak => para.add_run(Run::new().add_text(" ")),
        Inline::RawInline { content, .. } => {
            if crate::ir::doc::is_html_comment(content) {
                para
            } else {
                para.add_run(styled_run(content, style))
            }
        }
    }
}

fn add_inlines_styled(mut para: Paragraph, inlines: &[Inline], style: RunStyle) -> Paragraph {
    for il in inlines {
        para = add_inline_to_para(para, il, style);
    }
    para
}

fn add_run_to_hyperlink(hl: Hyperlink, il: &Inline, style: RunStyle) -> Hyperlink {
    match il {
        Inline::Text(t) => hl.add_run(styled_run(t, style)),
        Inline::Strong(inner) => add_runs_to_hyperlink(hl, inner, RunStyle { bold: true, ..style }),
        Inline::Emph(inner) => add_runs_to_hyperlink(hl, inner, RunStyle { italic: true, ..style }),
        Inline::Strikethrough(inner) => add_runs_to_hyperlink(hl, inner, RunStyle { strike: true, ..style }),
        Inline::Superscript(inner) => {
            add_runs_to_hyperlink(hl, inner, RunStyle { superscript: true, ..style })
        }
        Inline::Subscript(inner) => {
            add_runs_to_hyperlink(hl, inner, RunStyle { subscript: true, ..style })
        }
        Inline::Code(s) => hl.add_run(styled_run(s, RunStyle { code: true, ..style })),
        other => {
            let mut t = String::new();
            crate::ir::doc::inline_to_text(other, &mut t);
            if t.is_empty() { hl } else { hl.add_run(styled_run(&t, style)) }
        }
    }
}

fn add_runs_to_hyperlink(mut hl: Hyperlink, inlines: &[Inline], style: RunStyle) -> Hyperlink {
    for il in inlines {
        hl = add_run_to_hyperlink(hl, il, style);
    }
    hl
}

fn styled_run(text: &str, style: RunStyle) -> Run {
    let mut run = Run::new().add_text(text);
    if style.code { run = run.fonts(RunFonts::new().ascii("Courier New")); }
    // Build RunProperty once so fields don't overwrite each other.
    let mut rp = RunProperty::new();
    if style.bold        { rp = rp.bold(); }
    if style.italic      { rp = rp.italic(); }
    if style.strike      { rp = rp.strike(); }
    if style.superscript { rp = rp.vert_align(VertAlignType::SuperScript); }
    else if style.subscript { rp = rp.vert_align(VertAlignType::SubScript); }
    run.run_property = rp;
    run
}

fn try_embed_image(src: &str) -> Option<Vec<u8>> {
    if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:") {
        return None;
    }
    let path = Path::new(src);
    if path.exists() { std::fs::read(path).ok() } else { None }
}

fn read_image_dims(src: &str) -> Option<(u32, u32)> {
    let path = Path::new(src);
    if !path.exists() { return None; }
    let reader = image::io::Reader::open(path).ok()?;
    let reader = reader.with_guessed_format().ok()?;
    let (w_px, h_px) = reader.into_dimensions().ok()?;
    let w_emu = w_px * 9525;
    let h_emu = h_px * 9525;
    let max_w: u32 = 5486400;
    if w_emu > max_w {
        let scale = max_w as f64 / w_emu as f64;
        Some((max_w, (h_emu as f64 * scale) as u32))
    } else {
        Some((w_emu, h_emu))
    }
}
