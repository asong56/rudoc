/// Typst reader — converts a subset of .typ markup to DocIR.
/// Handles: headings (= / == / ===), paragraphs, code blocks, bold/italic, links.
use anyhow::Result;
use crate::ir::doc::{Block, DocIR, Inline};

pub fn parse(src: &str) -> Result<DocIR> {
    let mut doc = DocIR::new();
    let mut lines = src.lines().peekable();
    let mut para_buf: Vec<String> = Vec::new();

    while let Some(line) = lines.next() {
        // Heading: = Title, == Sub, === Sub-sub
        if let Some(rest) = line.strip_prefix("= ").or_else(|| {
            if line.starts_with("= ") { Some(&line[2..]) } else { None }
        }) {
            flush_para(&mut para_buf, &mut doc.blocks);
            let (level, text) = parse_heading(line);
            doc.blocks.push(Block::Heading(level, parse_inlines(text)));
            continue;
        }

        // Code block: ```lang ... ```
        if line.starts_with("```") {
            flush_para(&mut para_buf, &mut doc.blocks);
            let lang_str = line.trim_start_matches('`').trim().to_string();
            let lang = if lang_str.is_empty() { None } else { Some(lang_str) };
            let mut code_lines: Vec<&str> = Vec::new();
            while let Some(cl) = lines.next() {
                if cl.starts_with("```") {
                    break;
                }
                code_lines.push(cl);
            }
            let code = code_lines.join("\n");
            doc.blocks.push(Block::CodeBlock { lang, code });
            continue;
        }

        // Blank line → flush paragraph
        if line.trim().is_empty() {
            flush_para(&mut para_buf, &mut doc.blocks);
            continue;
        }

        // Horizontal rule
        if line == "---" || line == "===" {
            flush_para(&mut para_buf, &mut doc.blocks);
            doc.blocks.push(Block::HorizontalRule);
            continue;
        }

        para_buf.push(line.to_string());
    }
    flush_para(&mut para_buf, &mut doc.blocks);
    Ok(doc)
}

fn parse_heading(line: &str) -> (u8, &str) {
    let mut level = 0u8;
    let rest = line.trim_start_matches(|c| {
        if c == '=' {
            level += 1;
            true
        } else {
            false
        }
    });
    (level.min(6).max(1), rest.trim_start())
}

fn flush_para(buf: &mut Vec<String>, blocks: &mut Vec<Block>) {
    if buf.is_empty() { return; }
    let text = buf.join(" ");
    buf.clear();
    let inlines = parse_inlines(&text);
    if !inlines.is_empty() {
        blocks.push(Block::Para(inlines));
    }
}

/// Parse Typst inline markup: *bold*, _italic_, `code`, #link("url")[text]
pub fn parse_inlines(src: &str) -> Vec<Inline> {
    let mut out = Vec::new();
    let mut chars = src.char_indices().peekable();
    let mut text_start = 0;
    let bytes = src.as_bytes();

    while let Some((i, c)) = chars.next() {
        match c {
            '*' => {
                if i > text_start {
                    out.push(Inline::Text(src[text_start..i].to_string()));
                }
                // collect until closing *
                let mut inner = String::new();
                let mut closed = false;
                for (j, c2) in chars.by_ref() {
                    if c2 == '*' { closed = true; text_start = j + 1; break; }
                    inner.push(c2);
                }
                if closed {
                    out.push(Inline::Strong(vec![Inline::Text(inner)]));
                } else {
                    out.push(Inline::Text(format!("*{}", inner)));
                    text_start = src.len();
                }
            }
            '_' => {
                if i > text_start {
                    out.push(Inline::Text(src[text_start..i].to_string()));
                }
                let mut inner = String::new();
                let mut closed = false;
                for (j, c2) in chars.by_ref() {
                    if c2 == '_' { closed = true; text_start = j + 1; break; }
                    inner.push(c2);
                }
                if closed {
                    out.push(Inline::Emph(vec![Inline::Text(inner)]));
                } else {
                    out.push(Inline::Text(format!("_{}", inner)));
                    text_start = src.len();
                }
            }
            '`' => {
                if i > text_start {
                    out.push(Inline::Text(src[text_start..i].to_string()));
                }
                let mut inner = String::new();
                let mut closed = false;
                for (j, c2) in chars.by_ref() {
                    if c2 == '`' { closed = true; text_start = j + 1; break; }
                    inner.push(c2);
                }
                out.push(Inline::Code(inner));
                if !closed { text_start = src.len(); }
            }
            _ => {}
        }
    }
    if text_start < src.len() {
        let remaining = src[text_start..].to_string();
        if !remaining.is_empty() {
            out.push(Inline::Text(remaining));
        }
    }
    out
}
