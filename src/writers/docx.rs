use anyhow::Result;
use docx_rs::*;
use std::io::Cursor;

use crate::ir::doc::{Block, DocIR, Inline};

pub fn render(doc: &DocIR) -> Result<Vec<u8>> {
    let mut docx = Docx::new();

    if let Some(title) = &doc.metadata.title {
        let para = Paragraph::new()
            .add_run(Run::new().add_text(title).bold())
            .style("Title");
        docx = docx.add_paragraph(para);
    }

    for block in &doc.blocks {
        docx = add_block(docx, block);
    }

    let buf: Vec<u8> = Vec::new();
    let cursor = Cursor::new(buf);
    // pack returns ZipResult<()>; the cursor is consumed by the ZIP writer
    // We work around this by writing to a Vec<u8> via a wrapper
    let out = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    let out_clone = out.clone();

    struct VecWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
    impl std::io::Write for VecWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
    }
    impl std::io::Seek for VecWriter {
        fn seek(&mut self, _: std::io::SeekFrom) -> std::io::Result<u64> { Ok(0) }
    }

    // docx-rs pack requires Write + Seek; use the standard Cursor<Vec<u8>> approach
    // but we need to retrieve the vec after packing.
    // The simplest solution: use a shared Vec<u8> via Cursor, then get the inner vec.
    let shared_buf: Vec<u8> = Vec::new();
    let shared_cursor = Cursor::new(shared_buf);

    // Hack: we can't get the inner from a consumed cursor, so use a pre-sized approach.
    // Instead, write to a tmp file then read it back is too slow.
    // docx-rs build() returns XMLDocx which has pack(W: Write+Seek).
    // Use zip::ZipWriter directly on a Cursor and get it back.

    let output_buf = Vec::new();
    let output_cursor = Cursor::new(output_buf);
    let xml_docx = docx.build();
    xml_docx.pack(output_cursor).map_err(|e| anyhow::anyhow!("DOCX pack error: {}", e))?;

    // We can't get the bytes back from the consumed cursor after pack() takes ownership.
    // Solution: write to a NamedTempFile approach, or use a different writer.
    // Actual solution: use a shared Arc<Mutex<Vec<u8>>> wrapper that we can read after.

    // Re-implement using the real solution:
    let cell = std::cell::RefCell::new(Vec::<u8>::new());

    struct RefWriter<'a>(&'a std::cell::RefCell<Vec<u8>>, u64);
    impl<'a> std::io::Write for RefWriter<'a> {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            self.1 += buf.len() as u64;
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
    }
    impl<'a> std::io::Seek for RefWriter<'a> {
        fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
            // zip::ZipWriter needs Seek only for updating directory records.
            // For simple sequential writes, return current position.
            match pos {
                std::io::SeekFrom::Current(0) => Ok(self.1),
                _ => Ok(self.1),
            }
        }
    }

    // We need a fresh docx object because the first build() consumed it.
    // Let's rebuild.
    let bytes = build_docx_bytes(doc)?;
    Ok(bytes)
}

fn build_docx_bytes(doc: &DocIR) -> Result<Vec<u8>> {
    let mut docx = Docx::new();

    if let Some(title) = &doc.metadata.title {
        let para = Paragraph::new()
            .add_run(Run::new().add_text(title).bold())
            .style("Title");
        docx = docx.add_paragraph(para);
    }

    for block in &doc.blocks {
        docx = add_block(docx, block);
    }

    // Use a temp file approach: write to a NamedTempFile, read it back
    let tmp = std::env::temp_dir().join(format!("rudoc_{}.docx", std::process::id()));
    {
        let f = std::fs::File::create(&tmp)?;
        docx.build().pack(f).map_err(|e| anyhow::anyhow!("DOCX pack: {}", e))?;
    }
    let bytes = std::fs::read(&tmp)?;
    let _ = std::fs::remove_file(&tmp);
    Ok(bytes)
}

fn add_block(docx: Docx, block: &Block) -> Docx {
    match block {
        Block::Heading(level, inlines) => {
            let style = match level {
                1 => "Heading1",
                2 => "Heading2",
                3 => "Heading3",
                4 => "Heading4",
                5 => "Heading5",
                _ => "Heading6",
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
            for b in blocks { d = add_block(d, b); }
            d
        }
        Block::List { ordered, start, items, .. } => {
            let mut d = docx;
            let abstract_num = AbstractNumbering::new(1).add_level(Level::new(
                0,
                Start::new((*start).max(1) as usize),
                NumberFormat::new(if *ordered { "decimal" } else { "bullet" }),
                LevelText::new(if *ordered { "%1." } else { "•" }),
                LevelJc::new("left"),
            ));
            d = d.add_abstract_numbering(abstract_num);
            d = d.add_numbering(Numbering::new(1, 1));

            for item in items {
                let inlines = match item.first() {
                    Some(Block::Para(ils)) => ils.clone(),
                    Some(Block::Heading(_, ils)) => ils.clone(),
                    _ => vec![],
                };
                let mut para = Paragraph::new()
                    .numbering(NumberingId::new(1), IndentLevel::new(0));
                para = add_inlines_to_para(para, &inlines);
                d = d.add_paragraph(para);
                for b in item.iter().skip(1) { d = add_block(d, b); }
            }
            d
        }
        Block::Table { head, rows } => {
            let mut table = Table::new(vec![]);
            let mut header_cells = Vec::new();
            for cell_inlines in head {
                let mut para = Paragraph::new();
                para = add_inlines_to_para(para, cell_inlines);
                header_cells.push(TableCell::new().add_paragraph(para));
            }
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
            docx.add_paragraph(Paragraph::new().add_run(Run::new().add_text("─────────────────────────────────────")))
        }
        Block::RawBlock { content, .. } => {
            docx.add_paragraph(Paragraph::new().add_run(Run::new().add_text(content)))
        }
    }
}

fn add_inlines_to_para(mut para: Paragraph, inlines: &[Inline]) -> Paragraph {
    for il in inlines { para = add_inline_to_para(para, il); }
    para
}

fn add_inline_to_para(para: Paragraph, il: &Inline) -> Paragraph {
    match il {
        Inline::Text(t) => para.add_run(Run::new().add_text(t)),
        Inline::Strong(inner) => {
            let mut text = String::new();
            for i in inner { crate::ir::doc::inline_to_text(i, &mut text); }
            para.add_run(Run::new().add_text(&text).bold())
        }
        Inline::Emph(inner) => {
            let mut text = String::new();
            for i in inner { crate::ir::doc::inline_to_text(i, &mut text); }
            para.add_run(Run::new().add_text(&text).italic())
        }
        Inline::Strikethrough(inner) => {
            // docx-rs Run has no .strike(); apply via run_property workaround:
            // Use an underline style to approximate (strikethrough not directly on Run)
            let mut text = String::new();
            for i in inner { crate::ir::doc::inline_to_text(i, &mut text); }
            para.add_run(Run::new().add_text(format!("~~{}~~", text)))
        }
        Inline::Code(s) => {
            para.add_run(Run::new().add_text(s).fonts(RunFonts::new().ascii("Courier New")))
        }
        Inline::Link { url, content, .. } => {
            let mut text = String::new();
            for i in content { crate::ir::doc::inline_to_text(i, &mut text); }
            let hyperlink = Hyperlink::new(url, HyperlinkType::External)
                .add_run(Run::new().add_text(&text));
            para.add_hyperlink(hyperlink)
        }
        Inline::Image { src, alt } => {
            let mut alt_text = String::new();
            for i in alt { crate::ir::doc::inline_to_text(i, &mut alt_text); }
            para.add_run(Run::new().add_text(format!("[Image: {}]", alt_text)))
        }
        Inline::LineBreak => para.add_run(Run::new().add_break(BreakType::TextWrapping)),
        Inline::SoftBreak => para.add_run(Run::new().add_text(" ")),
        Inline::RawInline { content, .. } => para.add_run(Run::new().add_text(content)),
    }
}
