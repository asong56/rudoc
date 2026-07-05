use crate::ir::doc::{Block, DocIR, Inline};

pub fn render(doc: &DocIR, wrap_cols: Option<usize>) -> String {
    let mut out = String::new();
    for block in &doc.blocks {
        render_block(block, &mut out, 0);
        out.push('\n');
    }
    if let Some(cols) = wrap_cols {
        wrap_text(&out, cols)
    } else {
        out
    }
}

fn render_block(block: &Block, out: &mut String, depth: usize) {
    match block {
        Block::Heading(level, inlines) => {
            let indent = "  ".repeat(depth);
            out.push_str(&indent);
            let mut text = String::new();
            for il in inlines { inline_to_text(il, &mut text); }
            out.push_str(&text);
            out.push('\n');
            // Underline H1 and H2
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
            out.push_str(&indent);
            for il in inlines { inline_to_text(il, out); }
            out.push('\n');
        }
        Block::CodeBlock { code, .. } => {
            for line in code.lines() {
                out.push_str("    ");
                out.push_str(line);
                out.push('\n');
            }
        }
        Block::BlockQuote(blocks) => {
            for b in blocks {
                let mut inner = String::new();
                render_block(b, &mut inner, depth);
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
                            other => render_block(other, out, depth + 1),
                        }
                    } else {
                        render_block(block, out, depth + 1);
                    }
                }
            }
        }
        Block::Table { head, rows } => {
            // Simple ASCII table
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
            
            out.push_str(&sep);
            out.push('\n');
            // Header
            out.push('|');
            for (i, cell) in head.iter().enumerate() {
                let text = cell_text(cell);
                let w = col_widths[i];
                out.push_str(&format!(" {:<width$} |", text, width = w));
            }
            out.push('\n');
            out.push_str(&sep);
            out.push('\n');
            for row in rows {
                out.push('|');
                for (i, cell) in row.iter().enumerate() {
                    let text = cell_text(cell);
                    let w = col_widths.get(i).copied().unwrap_or(3);
                    out.push_str(&format!(" {:<width$} |", text, width = w));
                }
                out.push('\n');
            }
            out.push_str(&sep);
            out.push('\n');
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

fn wrap_text(text: &str, cols: usize) -> String {
    let mut out = String::new();
    for para in text.split("\n\n") {
        let words: Vec<&str> = para.split_whitespace().collect();
        let mut line_len = 0;
        for word in &words {
            if line_len + word.len() + 1 > cols && line_len > 0 {
                out.push('\n');
                line_len = 0;
            } else if line_len > 0 {
                out.push(' ');
                line_len += 1;
            }
            out.push_str(word);
            line_len += word.len();
        }
        out.push_str("\n\n");
    }
    out
}
