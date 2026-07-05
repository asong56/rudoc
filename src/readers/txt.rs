use anyhow::Result;
use crate::ir::doc::{Block, DocIR, Inline};

/// Parse plain text: split into paragraphs on blank lines.
pub fn parse(src: &str) -> Result<DocIR> {
    let mut doc = DocIR::new();
    let mut para_lines: Vec<&str> = Vec::new();

    for line in src.lines() {
        if line.trim().is_empty() {
            flush_para(&mut para_lines, &mut doc.blocks);
        } else {
            para_lines.push(line);
        }
    }
    flush_para(&mut para_lines, &mut doc.blocks);
    Ok(doc)
}

fn flush_para(lines: &mut Vec<&str>, blocks: &mut Vec<Block>) {
    if lines.is_empty() {
        return;
    }
    let text = lines.join(" ");
    lines.clear();
    if !text.trim().is_empty() {
        blocks.push(Block::Para(vec![Inline::Text(text)]));
    }
}
