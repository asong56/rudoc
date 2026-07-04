use anyhow::Result;
use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};

use crate::ir::doc::{Block, DocIR, Inline, Metadata};

pub fn parse(src: &str) -> Result<DocIR> {
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut src.as_bytes())?;

    let mut doc = DocIR::new();
    walk_node(&dom.document, &mut doc.blocks, &mut doc.metadata);
    Ok(doc)
}

fn walk_node(handle: &Handle, blocks: &mut Vec<Block>, meta: &mut Metadata) {
    match &handle.data {
        NodeData::Document => {
            for child in handle.children.borrow().iter() {
                walk_node(child, blocks, meta);
            }
        }
        NodeData::Element { name, attrs, .. } => {
            let tag = name.local.as_ref().to_lowercase();
            let attrs_map: std::collections::HashMap<String, String> = attrs
                .borrow()
                .iter()
                .map(|a| (a.name.local.as_ref().to_lowercase(), a.value.to_string()))
                .collect();

            match tag.as_str() {
                "html" | "body" | "div" | "article" | "section" | "main" | "header"
                | "footer" | "nav" | "aside" => {
                    for child in handle.children.borrow().iter() {
                        walk_node(child, blocks, meta);
                    }
                }
                "head" => {
                    // Extract title from <head>
                    for child in handle.children.borrow().iter() {
                        if let NodeData::Element { name, .. } = &child.data {
                            if name.local.as_ref() == "title" {
                                let text = extract_text(child);
                                if !text.is_empty() {
                                    meta.title = Some(text);
                                }
                            }
                        }
                    }
                }
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    let level = tag[1..].parse::<u8>().unwrap_or(1);
                    let inlines = node_to_inlines(handle);
                    if !inlines.is_empty() {
                        blocks.push(Block::Heading(level, inlines));
                    }
                }
                "p" => {
                    let inlines = node_to_inlines(handle);
                    if !inlines.is_empty() {
                        blocks.push(Block::Para(inlines));
                    }
                }
                "pre" => {
                    // Check for nested <code>
                    let text = extract_text(handle);
                    let lang = find_code_lang(handle);
                    blocks.push(Block::CodeBlock { lang, code: text });
                }
                "blockquote" => {
                    let mut inner = Vec::new();
                    for child in handle.children.borrow().iter() {
                        walk_node(child, &mut inner, meta);
                    }
                    if !inner.is_empty() {
                        blocks.push(Block::BlockQuote(inner));
                    }
                }
                "ul" | "ol" => {
                    let ordered = tag == "ol";
                    let mut items: Vec<Vec<Block>> = Vec::new();
                    for child in handle.children.borrow().iter() {
                        if let NodeData::Element { name, .. } = &child.data {
                            if name.local.as_ref() == "li" {
                                let mut item_blocks = Vec::new();
                                // Check if the li has block children
                                let has_block_children = child.children.borrow().iter().any(|c| {
                                    if let NodeData::Element { name, .. } = &c.data {
                                        let t = name.local.as_ref();
                                        matches!(t, "p" | "ul" | "ol" | "pre" | "blockquote")
                                    } else {
                                        false
                                    }
                                });
                                if has_block_children {
                                    for grandchild in child.children.borrow().iter() {
                                        walk_node(grandchild, &mut item_blocks, meta);
                                    }
                                } else {
                                    let inlines = node_to_inlines(child);
                                    if !inlines.is_empty() {
                                        item_blocks.push(Block::Para(inlines));
                                    }
                                }
                                items.push(item_blocks);
                            }
                        }
                    }
                    if !items.is_empty() {
                        blocks.push(Block::List {
                            ordered,
                            start: 1,
                            tight: true,
                            items,
                        });
                    }
                }
                "table" => {
                    let (head, rows) = parse_table(handle);
                    blocks.push(Block::Table { head, rows });
                }
                "hr" => {
                    blocks.push(Block::HorizontalRule);
                }
                "br" => {}
                // Inline elements at block level → wrap in Para
                _ => {
                    let inlines = node_to_inlines(handle);
                    if !inlines.is_empty() {
                        blocks.push(Block::Para(inlines));
                    }
                }
            }
        }
        NodeData::Text { contents } => {
            let text = contents.borrow().to_string();
            let trimmed = text.trim().to_string();
            if !trimmed.is_empty() {
                blocks.push(Block::Para(vec![Inline::Text(trimmed)]));
            }
        }
        _ => {}
    }
}

fn node_to_inlines(handle: &Handle) -> Vec<Inline> {
    let mut out = Vec::new();
    for child in handle.children.borrow().iter() {
        collect_inlines(child, &mut out);
    }
    out
}

fn collect_inlines(handle: &Handle, out: &mut Vec<Inline>) {
    match &handle.data {
        NodeData::Text { contents } => {
            let text = contents.borrow().to_string();
            if !text.is_empty() {
                out.push(Inline::Text(text));
            }
        }
        NodeData::Element { name, attrs, .. } => {
            let tag = name.local.as_ref().to_lowercase();
            match tag.as_str() {
                "em" | "i" => {
                    let inner = node_to_inlines(handle);
                    if !inner.is_empty() {
                        out.push(Inline::Emph(inner));
                    }
                }
                "strong" | "b" => {
                    let inner = node_to_inlines(handle);
                    if !inner.is_empty() {
                        out.push(Inline::Strong(inner));
                    }
                }
                "s" | "del" | "strike" => {
                    let inner = node_to_inlines(handle);
                    if !inner.is_empty() {
                        out.push(Inline::Strikethrough(inner));
                    }
                }
                "code" => {
                    let text = extract_text(handle);
                    out.push(Inline::Code(text));
                }
                "a" => {
                    let attrs_b = attrs.borrow();
                    let url = attrs_b
                        .iter()
                        .find(|a| a.name.local.as_ref() == "href")
                        .map(|a| a.value.to_string())
                        .unwrap_or_default();
                    let title = attrs_b
                        .iter()
                        .find(|a| a.name.local.as_ref() == "title")
                        .map(|a| a.value.to_string())
                        .unwrap_or_default();
                    let content = node_to_inlines(handle);
                    out.push(Inline::Link { url, title, content });
                }
                "img" => {
                    let attrs_b = attrs.borrow();
                    let src = attrs_b
                        .iter()
                        .find(|a| a.name.local.as_ref() == "src")
                        .map(|a| a.value.to_string())
                        .unwrap_or_default();
                    let alt_text = attrs_b
                        .iter()
                        .find(|a| a.name.local.as_ref() == "alt")
                        .map(|a| a.value.to_string())
                        .unwrap_or_default();
                    out.push(Inline::Image {
                        src,
                        alt: vec![Inline::Text(alt_text)],
                    });
                }
                "br" => out.push(Inline::LineBreak),
                "span" => {
                    for child in handle.children.borrow().iter() {
                        collect_inlines(child, out);
                    }
                }
                _ => {
                    for child in handle.children.borrow().iter() {
                        collect_inlines(child, out);
                    }
                }
            }
        }
        _ => {}
    }
}

fn extract_text(handle: &Handle) -> String {
    let mut out = String::new();
    for child in handle.children.borrow().iter() {
        extract_text_into(child, &mut out);
    }
    out
}

fn extract_text_into(handle: &Handle, out: &mut String) {
    match &handle.data {
        NodeData::Text { contents } => {
            out.push_str(&contents.borrow());
        }
        NodeData::Element { .. } => {
            for child in handle.children.borrow().iter() {
                extract_text_into(child, out);
            }
        }
        _ => {}
    }
}

fn find_code_lang(pre_handle: &Handle) -> Option<String> {
    for child in pre_handle.children.borrow().iter() {
        if let NodeData::Element { name, attrs, .. } = &child.data {
            if name.local.as_ref() == "code" {
                let attrs_b = attrs.borrow();
                if let Some(class) = attrs_b.iter().find(|a| a.name.local.as_ref() == "class") {
                    let cls = class.value.to_string();
                    // "language-rust" → "rust"
                    if let Some(lang) = cls.strip_prefix("language-") {
                        return Some(lang.to_string());
                    }
                    return Some(cls);
                }
            }
        }
    }
    None
}

fn parse_table(table: &Handle) -> (Vec<Vec<Inline>>, Vec<Vec<Vec<Inline>>>) {
    let mut head: Vec<Vec<Inline>> = Vec::new();
    let mut rows: Vec<Vec<Vec<Inline>>> = Vec::new();

    for child in table.children.borrow().iter() {
        if let NodeData::Element { name, .. } = &child.data {
            match name.local.as_ref() {
                "thead" => {
                    for row in child.children.borrow().iter() {
                        if is_tag(row, "tr") {
                            head = collect_row_cells(row);
                        }
                    }
                }
                "tbody" | "tfoot" => {
                    for row in child.children.borrow().iter() {
                        if is_tag(row, "tr") {
                            rows.push(collect_row_cells(row));
                        }
                    }
                }
                "tr" => {
                    let cells = collect_row_cells(child);
                    if head.is_empty() {
                        head = cells;
                    } else {
                        rows.push(cells);
                    }
                }
                _ => {}
            }
        }
    }
    (head, rows)
}

fn collect_row_cells(row: &Handle) -> Vec<Vec<Inline>> {
    let mut cells = Vec::new();
    for child in row.children.borrow().iter() {
        if let NodeData::Element { name, .. } = &child.data {
            let tag = name.local.as_ref();
            if tag == "td" || tag == "th" {
                cells.push(node_to_inlines(child));
            }
        }
    }
    cells
}

fn is_tag(handle: &Handle, tag: &str) -> bool {
    if let NodeData::Element { name, .. } = &handle.data {
        name.local.as_ref() == tag
    } else {
        false
    }
}
