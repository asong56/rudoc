use crate::ir::doc::{Block, DocIR, Inline};

pub fn render(doc: &DocIR, wrap: bool) -> String {
    let mut out = String::new();

    // YAML front matter if metadata present
    if doc.metadata.title.is_some() || doc.metadata.author.is_some() {
        out.push_str("---\n");
        if let Some(t) = &doc.metadata.title {
            out.push_str(&format!("title: \"{}\"\n", t.replace('"', "\\\"")));
        }
        if let Some(a) = &doc.metadata.author {
            out.push_str(&format!("author: \"{}\"\n", a));
        }
        if let Some(d) = &doc.metadata.date {
            out.push_str(&format!("date: \"{}\"\n", d));
        }
        out.push_str("---\n\n");
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
            out.push_str(&"#".repeat(*level as usize));
            out.push(' ');
            render_inlines(inlines, out);
            out.push('\n');
        }
        Block::Para(inlines) => {
            render_inlines(inlines, out);
            out.push('\n');
        }
        Block::CodeBlock { lang, code } => {
            out.push_str("```");
            if let Some(l) = lang {
                out.push_str(l);
            }
            out.push('\n');
            out.push_str(code);
            if !code.ends_with('\n') { out.push('\n'); }
            out.push_str("```\n");
        }
        Block::BlockQuote(blocks) => {
            let mut inner = String::new();
            for b in blocks {
                render_block(b, &mut inner, depth + 1);
                inner.push('\n');
            }
            for line in inner.lines() {
                out.push_str("> ");
                out.push_str(line);
                out.push('\n');
            }
        }
        Block::List { ordered, start, items, .. } => {
            for (i, item) in items.iter().enumerate() {
                let prefix = if *ordered {
                    format!("{}. ", i as u64 + start)
                } else {
                    "- ".to_string()
                };
                let mut first = true;
                for block in item {
                    if first {
                        out.push_str(&prefix);
                        first = false;
                        match block {
                            Block::Para(inlines) => {
                                render_inlines(inlines, out);
                                out.push('\n');
                            }
                            other => {
                                let mut inner = String::new();
                                render_block(other, &mut inner, depth + 1);
                                out.push_str(inner.trim_start());
                            }
                        }
                    } else {
                        let indent = " ".repeat(prefix.len());
                        let mut inner = String::new();
                        render_block(block, &mut inner, depth + 1);
                        for line in inner.lines() {
                            out.push_str(&indent);
                            out.push_str(line);
                            out.push('\n');
                        }
                    }
                }
            }
        }
        Block::Table { head, rows } => {
            // Header row
            out.push('|');
            for cell in head {
                out.push(' ');
                render_inlines(cell, out);
                out.push_str(" |");
            }
            out.push('\n');
            // Separator
            out.push('|');
            for _ in head {
                out.push_str(" --- |");
            }
            out.push('\n');
            // Data rows
            for row in rows {
                out.push('|');
                for cell in row {
                    out.push(' ');
                    render_inlines(cell, out);
                    out.push_str(" |");
                }
                out.push('\n');
            }
        }
        Block::HorizontalRule => out.push_str("---\n"),
        Block::RawBlock { content, .. } => out.push_str(content),
    }
}

fn render_inlines(inlines: &[Inline], out: &mut String) {
    for il in inlines {
        render_inline(il, out);
    }
}

fn render_inline(il: &Inline, out: &mut String) {
    match il {
        Inline::Text(t) => out.push_str(t),
        Inline::Emph(inner) => {
            out.push('*');
            render_inlines(inner, out);
            out.push('*');
        }
        Inline::Strong(inner) => {
            out.push_str("**");
            render_inlines(inner, out);
            out.push_str("**");
        }
        Inline::Strikethrough(inner) => {
            out.push_str("~~");
            render_inlines(inner, out);
            out.push_str("~~");
        }
        Inline::Code(s) => {
            out.push('`');
            out.push_str(s);
            out.push('`');
        }
        Inline::Link { url, title, content } => {
            out.push('[');
            render_inlines(content, out);
            out.push_str("](");
            out.push_str(url);
            if !title.is_empty() {
                out.push_str(&format!(" \"{}\"", title));
            }
            out.push(')');
        }
        Inline::Image { src, alt } => {
            out.push_str("![");
            render_inlines(alt, out);
            out.push_str("](");
            out.push_str(src);
            out.push(')');
        }
        Inline::LineBreak => out.push_str("  \n"),
        Inline::SoftBreak => out.push(' '),
        Inline::RawInline { content, .. } => out.push_str(content),
    }
}
