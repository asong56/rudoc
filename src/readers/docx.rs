use anyhow::{Context, Result};
use quick_xml::events::Event as XmlEvent;
use quick_xml::Reader;
use std::io::Read;

use crate::ir::doc::{Block, DocIR, Inline, Metadata};

pub fn parse(bytes: &[u8]) -> Result<DocIR> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).context("Not a valid DOCX (ZIP) file")?;

    // Read word/document.xml
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
    let mut in_para = false;
    let mut current_inlines: Vec<Inline> = Vec::new();
    let mut current_run_text = String::new();
    let mut run_bold = false;
    let mut run_italic = false;
    let mut run_strike = false;
    let mut run_code = false;
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
                    "w:pStyle" => {
                        // Style name is in val attribute
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" {
                                para_style = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                    "w:b" => run_bold = true,
                    "w:i" => run_italic = true,
                    "w:strike" | "w:dstrike" => run_strike = true,
                    "w:rStyle" => {
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
                    "w:r" => {
                        // End of run: flush with formatting
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
                            current_inlines.push(inline);
                        }
                        run_bold = false;
                        run_italic = false;
                        run_strike = false;
                        run_code = false;
                    }
                    "w:p" => {
                        if in_para {
                            let inlines = std::mem::take(&mut current_inlines);
                            if !inlines.is_empty() {
                                let block = style_to_block(&para_style, inlines);
                                blocks.push(block);
                            }
                            in_para = false;
                        }
                    }
                    "w:body" => break,
                    _ => {}
                }
            }
            Ok(XmlEvent::Text(ref e)) => {
                if in_para {
                    current_run_text.push_str(&e.unescape().unwrap_or_default());
                }
            }
            Ok(XmlEvent::Empty(ref e)) => {
                let name = std::str::from_utf8(e.name().as_ref()).unwrap_or("").to_string();
                match name.as_str() {
                    "w:br" => { current_inlines.push(Inline::LineBreak); }
                    "w:pStyle" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" {
                                para_style = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                    "w:b" => { run_bold = true; }
                    "w:i" => { run_italic = true; }
                    _ => {}
                }
            }
            Ok(XmlEvent::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    doc.blocks = blocks;
    Ok(())
}

fn style_to_block(style: &str, inlines: Vec<Inline>) -> Block {
    match style {
        "Heading1" | "heading1" | "1" => Block::Heading(1, inlines),
        "Heading2" | "heading2" | "2" => Block::Heading(2, inlines),
        "Heading3" | "heading3" | "3" => Block::Heading(3, inlines),
        "Heading4" | "heading4" | "4" => Block::Heading(4, inlines),
        "Heading5" | "heading5" | "5" => Block::Heading(5, inlines),
        "Heading6" | "heading6" | "6" => Block::Heading(6, inlines),
        "VerbatimBlock" | "SourceCode" | "Code" => {
            let text: String = inlines.iter().map(|i| match i {
                Inline::Text(t) | Inline::Code(t) => t.as_str(),
                _ => "",
            }).collect();
            Block::CodeBlock { lang: None, code: text }
        }
        _ => Block::Para(inlines),
    }
}
