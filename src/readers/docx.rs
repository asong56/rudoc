use anyhow::{Context, Result};
use quick_xml::events::Event as XmlEvent;
use quick_xml::Reader;
use std::io::Read;

use crate::ir::doc::{Block, DocIR, Inline};

#[derive(PartialEq, Clone, Copy)]
enum VertAlign { None, Superscript, Subscript }

pub fn parse(bytes: &[u8]) -> Result<DocIR> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).context("Not a valid DOCX (ZIP) file")?;

    let xml = {
        let mut f = archive
            .by_name("word/document.xml")
            .context("Missing word/document.xml")?;
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        s
    };

    let mut doc = DocIR::new();
    parse_document_xml(&xml, &mut doc)?;
    Ok(doc)
}

fn parse_document_xml(xml: &str, doc: &mut DocIR) -> Result<()> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(false);

    let mut buf = Vec::new();
    let mut blocks: Vec<Block> = Vec::new();

    // State
    let mut in_para      = false;
    let mut in_p_pr       = false; // inside <w:pPr>
    let mut in_r_pr       = false; // inside <w:rPr>
    let mut current_inlines: Vec<Inline> = Vec::new();
    let mut current_run_text = String::new();
    let mut run_bold   = false;
    let mut run_italic = false;
    let mut run_strike = false;
    let mut run_code   = false;
    let mut run_vert_align = VertAlign::None;
    let mut para_style = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Start(ref e)) => {
                let name = std::str::from_utf8(e.name().as_ref()).unwrap_or("").to_string();
                match name.as_str() {
                    "w:p" => {
                        in_para = true;
                        current_inlines.clear();
                        para_style.clear();
                    }
                    "w:pPr" => { in_p_pr = true; }
                    "w:rPr" => { in_r_pr = true; }
                    // pStyle lives inside w:pPr — only capture it there
                    "w:pStyle" if in_p_pr => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" {
                                para_style = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                    "w:b"  if in_r_pr => run_bold   = true,
                    "w:i"  if in_r_pr => run_italic  = true,
                    "w:strike" | "w:dstrike" if in_r_pr => run_strike = true,
                    "w:vertAlign" if in_r_pr => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" {
                                run_vert_align = match attr.value.as_ref() {
                                    b"superscript" => VertAlign::Superscript,
                                    b"subscript" => VertAlign::Subscript,
                                    _ => VertAlign::None,
                                };
                            }
                        }
                    }
                    "w:rStyle" if in_r_pr => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" {
                                let style = String::from_utf8_lossy(&attr.value).to_string();
                                if style.to_lowercase().contains("code") || style == "VerbatimChar" {
                                    run_code = true;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(XmlEvent::End(ref e)) => {
                let name = std::str::from_utf8(e.name().as_ref()).unwrap_or("").to_string();
                match name.as_str() {
                    "w:pPr" => { in_p_pr = false; }
                    "w:rPr" => { in_r_pr = false; }
                    "w:r" => {
                        if !current_run_text.is_empty() {
                            let text = std::mem::take(&mut current_run_text);
                            let mut inline = if run_code {
                                Inline::Code(text)
                            } else {
                                Inline::Text(text)
                            };
                            if run_bold && !run_code {
                                inline = Inline::Strong(vec![inline]);
                            }
                            if run_italic && !run_code {
                                inline = Inline::Emph(vec![inline]);
                            }
                            if run_strike && !run_code {
                                inline = Inline::Strikethrough(vec![inline]);
                            }
                            match run_vert_align {
                                VertAlign::Superscript => inline = Inline::Superscript(vec![inline]),
                                VertAlign::Subscript => inline = Inline::Subscript(vec![inline]),
                                VertAlign::None => {}
                            }
                            current_inlines.push(inline);
                        }
                        run_bold   = false;
                        run_italic = false;
                        run_strike = false;
                        run_code   = false;
                        run_vert_align = VertAlign::None;
                    }
                    "w:p" => {
                        if in_para {
                            let inlines = std::mem::take(&mut current_inlines);
                            if !inlines.is_empty() {
                                let block = style_to_block(&para_style, inlines);
                                blocks.push(block);
                            }
                            in_para = false;
                            in_p_pr  = false;
                        }
                    }
                    "w:body" => break,
                    _ => {}
                }
            }
            Ok(XmlEvent::Text(ref e)) => {
                if in_para && !in_p_pr {
                    current_run_text.push_str(&e.unescape().unwrap_or_default());
                }
            }
            Ok(XmlEvent::Empty(ref e)) => {
                let name = std::str::from_utf8(e.name().as_ref()).unwrap_or("").to_string();
                match name.as_str() {
                    "w:br" => { current_inlines.push(Inline::LineBreak); }
                    // Self-closing pStyle (some exporters emit it this way)
                    "w:pStyle" if in_p_pr => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" {
                                para_style = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                    "w:b"  if in_r_pr => { run_bold   = true; }
                    "w:i"  if in_r_pr => { run_italic  = true; }
                    "w:strike" | "w:dstrike" if in_r_pr => { run_strike = true; }
                    "w:vertAlign" if in_r_pr => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" {
                                run_vert_align = match attr.value.as_ref() {
                                    b"superscript" => VertAlign::Superscript,
                                    b"subscript" => VertAlign::Subscript,
                                    _ => VertAlign::None,
                                };
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(XmlEvent::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML parse error in document.xml: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    doc.blocks = blocks;
    Ok(())
}

/// Map a paragraph style name to the correct IR Block.
/// Handles: "Heading1", "Heading 1" (Word default), "heading1",
/// localised names via numeric suffix ("1"–"6"), and code block styles.
fn style_to_block(style: &str, inlines: Vec<Inline>) -> Block {
    // Normalise: lowercase, strip spaces/hyphens so "Heading 1" == "heading1"
    let norm: String = style.chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect::<String>()
        .to_lowercase();

    // "heading1" .. "heading6"  or bare "1" .. "6"
    let heading_level: Option<u8> = match norm.as_str() {
        "heading1" | "1" => Some(1),
        "heading2" | "2" => Some(2),
        "heading3" | "3" => Some(3),
        "heading4" | "4" => Some(4),
        "heading5" | "5" => Some(5),
        "heading6" | "6" => Some(6),
        // "überschrift1", "titre1", "overskrift1", etc. — ends with a digit 1–6
        other if other.ends_with(|c: char| c.is_ascii_digit()) => {
            let last = other.chars().last().unwrap() as u8 - b'0';
            if (1..=6).contains(&last) { Some(last) } else { None }
        }
        _ => None,
    };

    if let Some(level) = heading_level {
        return Block::Heading(level, inlines);
    }

    match norm.as_str() {
        "verbatimblock" | "sourcecode" | "code" | "codeblock" => {
            let text: String = inlines.iter().map(|i| match i {
                Inline::Text(t) | Inline::Code(t) => t.as_str(),
                _ => "",
            }).collect();
            Block::CodeBlock { lang: None, code: text }
        }
        _ => Block::Para(inlines),
    }
}
