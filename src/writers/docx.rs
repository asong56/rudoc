//! DOCX writer using docx-rs.
//! Images are embedded as actual binary parts in the ZIP.
use anyhow::Result;
use docx_rs::*;

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

    // Numbering counter (each list gets its own ID)
    let mut num_id_counter = 1usize;

    for block in &doc.blocks {
        (docx, num_id_counter) = add_block(docx, block, num_id_counter)?;
    }

    let tmp = std::env::temp_dir()
        .join(format!("rudoc_{}_{}.docx", std::process::id(), rand_suffix()));
    {
        let f = std::fs::File::create(&tmp)?;
        docx.build().pack(f).map_err(|e| anyhow::anyhow!("DOCX pack: {}", e))?;
    }
    let bytes = std::fs::read(&tmp)?;
    let _ = std::fs::remove_file(&tmp);
    Ok(bytes)
}

fn add_block(docx: Docx, block: &Block, num_id: usize)
    -> Result<(Docx, usize)>
{
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
                // Sub-blocks in list item
                for sub in item.iter().skip(1) {
                    (d, num_id) = add_block(d, sub, num_id + 1)?;
                    num_id -= 1; // restore after sub-block
                }
            }
            num_id += 1; // each list gets a unique numbering ID
            d
        }
        Block::Table { head, rows } => {
            let mut table = Table::new(vec![]);
            let header_cells: Vec<TableCell> = head.iter().map(|cell_inlines| {
                let mut para = Paragraph::new();
                para = add_inlines_to_para(para, cell_inlines);
                // Bold header cells via wrapping run
                let mut p2 = Paragraph::new();
                p2 = p2.add_run(Run::new().add_text(
                    cell_inlines.iter().map(|i| {
                        let mut s = String::new();
                        crate::ir::doc::inline_to_text(i, &mut s); s
                    }).collect::<String>()
                ).bold());
                TableCell::new().add_paragraph(p2)
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
                Paragraph::new().add_run(Run::new().add_text(
                    "─".repeat(40)
                ))
            )
        }
        Block::RawBlock { content, .. } => {
            docx.add_paragraph(Paragraph::new().add_run(Run::new().add_text(content)))
        }
    };
    Ok((docx, num_id))
}

fn add_inlines_to_para(mut para: Paragraph, inlines: &[Inline]) -> Paragraph {
    for il in inlines { para = add_inline_to_para(para, il); }
    para
}

fn add_inline_to_para(para: Paragraph, il: &Inline) -> Paragraph {
    match il {
        Inline::Text(t) => para.add_run(Run::new().add_text(t)),
        Inline::Strong(inner) => {
            let mut t = String::new();
            for i in inner { crate::ir::doc::inline_to_text(i, &mut t); }
            para.add_run(Run::new().add_text(&t).bold())
        }
        Inline::Emph(inner) => {
            let mut t = String::new();
            for i in inner { crate::ir::doc::inline_to_text(i, &mut t); }
            para.add_run(Run::new().add_text(&t).italic())
        }
        Inline::Strikethrough(inner) => {
            let mut t = String::new();
            for i in inner { crate::ir::doc::inline_to_text(i, &mut t); }
            // docx-rs Run doesn't expose strike directly; embed via run_property path
            para.add_run(Run::new().add_text(format!("~~{}~~", t)))
        }
        Inline::Code(s) => {
            para.add_run(
                Run::new().add_text(s).fonts(RunFonts::new().ascii("Courier New"))
            )
        }
        Inline::Link { url, content, .. } => {
            let mut t = String::new();
            for i in content { crate::ir::doc::inline_to_text(i, &mut t); }
            let hl = Hyperlink::new(url, HyperlinkType::External)
                .add_run(Run::new().add_text(&t));
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
        Inline::RawInline { content, .. } => para.add_run(Run::new().add_text(content)),
    }
}

/// Try to read local image file into bytes.
/// Returns None for URLs (http/https) or missing files.
fn try_embed_image(src: &str) -> Option<Vec<u8>> {
    // Skip URLs
    if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:") {
        return None;
    }
    let path = Path::new(src);
    if path.exists() {
        std::fs::read(path).ok()
    } else {
        None
    }
}

/// Get image width in EMU (English Metric Units: 1 inch = 914400 EMU).
/// Reads actual image dimensions, falls back to 3 inches wide.

fn read_image_dims(src: &str) -> Option<(u32, u32)> {
    let path = Path::new(src);
    if !path.exists() { return None; }
    let reader = image::io::Reader::open(path).ok()?;
    let reader = reader.with_guessed_format().ok()?;
    let (w_px, h_px) = reader.into_dimensions().ok()?;
    // Convert pixels to EMU assuming 96 DPI: 1 px = 914400/96 = 9525 EMU
    let w_emu = w_px * 9525;
    let h_emu = h_px * 9525;
    // Cap at page width (6 inches = 5486400 EMU) maintaining aspect ratio
    let max_w: u32 = 5486400;
    if w_emu > max_w {
        let scale = max_w as f64 / w_emu as f64;
        Some((max_w, (h_emu as f64 * scale) as u32))
    } else {
        Some((w_emu, h_emu))
    }
}

fn rand_suffix() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(42)
}
