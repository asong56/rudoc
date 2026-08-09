use crate::ir::doc::{Block, DocIR, Inline};

/// Render DocIR to Typst source (.typ).
pub fn render(doc: &DocIR, paper: &str, font: &str) -> String {
    let mut out = String::new();

    // Document setup
    out.push_str(&format!(
        "#set page(paper: \"{}\")\n#set text(font: \"{}\", size: 11pt)\n#set par(justify: true)\n\n",
        paper, font
    ));

    // Metadata
    if let Some(title) = &doc.metadata.title {
        out.push_str(&format!(
            "#align(center, text(size: 18pt, weight: \"bold\")[{}])\n\n",
            escape_typ(title)
        ));
    }
    if let Some(author) = &doc.metadata.author {
        out.push_str(&format!(
            "#align(center)[{}]\n\n",
            escape_typ(author)
        ));
    }
    if let Some(date) = &doc.metadata.date {
        out.push_str(&format!(
            "#align(center)[{}]\n\n",
            escape_typ(date)
        ));
    }

    for block in &doc.blocks {
        render_block(block, &mut out, 0);
        out.push('\n');
    }
    out
}

fn render_block(block: &Block, out: &mut String, depth: usize) {
    match block {
        Block::Heading(level, inlines) => {
            let prefix = match level {
                1 => "= ",
                2 => "== ",
                3 => "=== ",
                4 => "==== ",
                5 => "===== ",
                _ => "====== ",
            };
            out.push_str(prefix);
            render_inlines(inlines, out);
            out.push('\n');
        }
        Block::Para(inlines) => {
            render_inlines(inlines, out);
            out.push('\n');
        }
        Block::CodeBlock { lang, code } => {
            let lang_str = lang.as_deref().unwrap_or("");
            out.push_str(&format!("```{}\n{}\n```\n", lang_str, code));
        }
        Block::BlockQuote(blocks) => {
            out.push_str("#block(stroke: (left: 2pt + gray), inset: (left: 8pt, y: 4pt))[\n");
            for b in blocks { render_block(b, out, depth + 1); }
            out.push_str("]\n");
        }
        Block::List { ordered, start, items, .. } => {
            for (i, item) in items.iter().enumerate() {
                if *ordered {
                    out.push_str(&format!("{}. ", i as u64 + start));
                } else {
                    out.push_str("- ");
                }
                for (bi, block) in item.iter().enumerate() {
                    if bi == 0 {
                        match block {
                            Block::Para(inlines) => {
                                render_inlines(inlines, out);
                                out.push('\n');
                            }
                            other => render_block(other, out, depth + 1),
                        }
                    } else {
                        render_block(block, out, depth + 1);
                    }
                }
            }
        }
        Block::Table { head, rows } => {
            let cols = head.len().max(rows.first().map(|r| r.len()).unwrap_or(1));
            out.push_str(&format!(
                "#table(\n  columns: {},\n  table.header(",
                cols
            ));
            for cell in head {
                out.push_str("[*");
                render_inlines(cell, out);
                out.push_str("*], ");
            }
            out.push_str("),\n");
            for row in rows {
                for cell in row {
                    out.push_str("  [");
                    render_inlines(cell, out);
                    out.push_str("],\n");
                }
            }
            out.push_str(")\n");
        }
        Block::HorizontalRule => out.push_str("#line(length: 100%)\n"),
        Block::RawBlock { content, .. } => out.push_str(content),
    }
}

fn render_inlines(inlines: &[Inline], out: &mut String) {
    for il in inlines { render_inline(il, out); }
}

fn render_inline(il: &Inline, out: &mut String) {
    match il {
        Inline::Text(t) => out.push_str(&escape_typ(t)),
        Inline::Emph(inner) => {
            out.push_str("_");
            render_inlines(inner, out);
            out.push('_');
        }
        Inline::Strong(inner) => {
            out.push_str("*");
            render_inlines(inner, out);
            out.push('*');
        }
        Inline::Strikethrough(inner) => {
            out.push_str("#strike[");
            render_inlines(inner, out);
            out.push(']');
        }
        Inline::Code(s) => {
            out.push('`');
            out.push_str(s);
            out.push('`');
        }
        Inline::Link { url, content, .. } => {
            out.push_str(&format!("#link(\"{}\")[", escape_typ(url)));
            render_inlines(content, out);
            out.push(']');
        }
        Inline::Image { src, alt } => {
            let mut alt_text = String::new();
            for il in alt { crate::ir::doc::inline_to_text(il, &mut alt_text); }
            out.push_str(&format!("#image(\"{}\", alt: \"{}\")", escape_typ(src), escape_typ(&alt_text)));
        }
        Inline::Superscript(inner) => {
            out.push_str("#super[");
            render_inlines(inner, out);
            out.push(']');
        }
        Inline::Subscript(inner) => {
            out.push_str("#sub[");
            render_inlines(inner, out);
            out.push(']');
        }
        Inline::LineBreak => out.push_str("\\\n"),
        Inline::SoftBreak => out.push(' '),
        Inline::RawInline { content, .. } => {
            if !crate::ir::doc::is_html_comment(content) {
                out.push_str(content);
            }
        }
    }
}

/// Escape Typst special characters in running text.
fn escape_typ(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('#', "\\#")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('@', "\\@")
        .replace('$', "\\$")
}
