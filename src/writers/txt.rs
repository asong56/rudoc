use crate::ir::doc::{Block, DocIR, Inline};

pub fn render(doc: &DocIR, wrap_cols: Option<usize>) -> String {
    let mut out = String::new();
    for block in &doc.blocks {
        render_block(block, &mut out, 0, wrap_cols);
        out.push('\n');
    }
    out
}

fn render_block(block: &Block, out: &mut String, depth: usize, wrap_cols: Option<usize>) {
    match block {
        Block::Heading(level, inlines) => {
            let indent = "  ".repeat(depth);
            out.push_str(&indent);
            let mut text = String::new();
            for il in inlines { inline_to_text(il, &mut text); }
            out.push_str(&text);
            out.push('\n');
            if *level == 1 {
                out.push_str(&indent);
                out.push_str(&"=".repeat(text.len().min(72)));
                out.push('\n');
            } else if *level == 2 {
                out.push_str(&indent);
                out.push_str(&"-".repeat(text.len().min(72)));
                out.push('\n');
            }
        }
        Block::Para(inlines) => {
            let indent = "  ".repeat(depth);
            let mut text = String::new();
            for il in inlines { inline_to_text(il, &mut text); }
            // Apply wrapping per-paragraph, preserving indent
            if let Some(cols) = wrap_cols {
                let effective = cols.saturating_sub(indent.len());
                for line in wrap_paragraph(&text, effective) {
                    out.push_str(&indent);
                    out.push_str(&line);
                    out.push('\n');
                }
            } else {
                out.push_str(&indent);
                out.push_str(&text);
                out.push('\n');
            }
        }
        Block::CodeBlock { code, .. } => {
            // Code blocks are never wrapped — preserve exact content
            for line in code.lines() {
                out.push_str("    ");
                out.push_str(line);
                out.push('\n');
            }
        }
        Block::BlockQuote(blocks) => {
            for b in blocks {
                let mut inner = String::new();
                render_block(b, &mut inner, depth, wrap_cols);
                for line in inner.lines() {
                    out.push_str("  > ");
                    out.push_str(line);
                    out.push('\n');
                }
            }
        }
        Block::List { ordered, start, items, .. } => {
            let indent = "  ".repeat(depth);
            for (i, item) in items.iter().enumerate() {
                let prefix = if *ordered {
                    format!("{}{}. ", indent, i as u64 + start)
                } else {
                    format!("{}- ", indent)
                };
                for (bi, block) in item.iter().enumerate() {
                    if bi == 0 {
                        out.push_str(&prefix);
                        match block {
                            Block::Para(inlines) => {
                                for il in inlines { inline_to_text(il, out); }
                                out.push('\n');
                            }
                            other => render_block(other, out, depth + 1, wrap_cols),
                        }
                    } else {
                        render_block(block, out, depth + 1, wrap_cols);
                    }
                }
            }
        }
        Block::Table { head, rows } => {
            let col_widths: Vec<usize> = (0..head.len())
                .map(|i| {
                    let header_len = cell_text(&head[i]).len();
                    let max_data = rows.iter()
                        .filter_map(|r| r.get(i))
                        .map(|c| cell_text(c).len())
                        .max()
                        .unwrap_or(0);
                    header_len.max(max_data).max(3)
                })
                .collect();

            let sep: String = col_widths.iter()
                .map(|w| "+".to_string() + &"-".repeat(w + 2))
                .collect::<String>() + "+";

            out.push_str(&sep); out.push('\n');
            out.push('|');
            for (i, cell) in head.iter().enumerate() {
                let text = cell_text(cell);
                let w = col_widths[i];
                out.push_str(&format!(" {:<width$} |", text, width = w));
            }
            out.push('\n');
            out.push_str(&sep); out.push('\n');
            for row in rows {
                out.push('|');
                for (i, cell) in row.iter().enumerate() {
                    let text = cell_text(cell);
                    let w = col_widths.get(i).copied().unwrap_or(3);
                    out.push_str(&format!(" {:<width$} |", text, width = w));
                }
                out.push('\n');
            }
            out.push_str(&sep); out.push('\n');
        }
        Block::HorizontalRule => {
            out.push_str(&"-".repeat(72));
            out.push('\n');
        }
        Block::RawBlock { content, .. } => {
            out.push_str(content);
            out.push('\n');
        }
    }
}

fn cell_text(inlines: &[Inline]) -> String {
    let mut s = String::new();
    for il in inlines { inline_to_text(il, &mut s); }
    s
}

fn inline_to_text(il: &Inline, out: &mut String) {
    crate::ir::doc::inline_to_text(il, out);
}

/// Word-wrap a single paragraph into lines of at most `cols` chars.
fn wrap_paragraph(text: &str, cols: usize) -> Vec<String> {
    if cols == 0 { return vec![text.to_string()]; }
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.len() + 1 + word.len() > cols {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        } else {
            current.push(' ');
            current.push_str(word);
        }
    }
    if !current.is_empty() { lines.push(current); }
    if lines.is_empty() { lines.push(String::new()); }
    lines
}
