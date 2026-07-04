use crate::ir::doc::{Block, DocIR, Inline};

pub fn render(doc: &DocIR, standalone: bool) -> String {
    let mut body = String::new();
    for block in &doc.blocks {
        render_block(block, &mut body);
        body.push('\n');
    }

    if !standalone {
        return body;
    }

    let title = doc.metadata.title.as_deref().unwrap_or("Document");
    format!(
        r#"<!DOCTYPE html>
<html lang="{lang}">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title}</title>
<style>
  body {{ max-width: 860px; margin: 2rem auto; font-family: system-ui, sans-serif;
          line-height: 1.6; color: #222; padding: 0 1rem; }}
  pre {{ background: #f5f5f5; padding: 1rem; border-radius: 4px; overflow-x: auto; }}
  code {{ font-family: "Fira Code", "Cascadia Code", monospace; font-size: 0.9em; }}
  blockquote {{ border-left: 4px solid #ccc; margin: 0; padding-left: 1rem; color: #555; }}
  table {{ border-collapse: collapse; width: 100%; }}
  th, td {{ border: 1px solid #ddd; padding: 0.5rem 0.75rem; text-align: left; }}
  th {{ background: #f0f0f0; }}
  img {{ max-width: 100%; }}
  hr {{ border: none; border-top: 1px solid #eee; }}
</style>
</head>
<body>
{body}
</body>
</html>
"#,
        lang = doc.metadata.lang.as_deref().unwrap_or("en"),
        title = escape_html(title),
        body = body,
    )
}

fn render_block(block: &Block, out: &mut String) {
    match block {
        Block::Heading(level, inlines) => {
            out.push_str(&format!("<h{}>", level));
            render_inlines(inlines, out);
            out.push_str(&format!("</h{}>\n", level));
        }
        Block::Para(inlines) => {
            out.push_str("<p>");
            render_inlines(inlines, out);
            out.push_str("</p>\n");
        }
        Block::CodeBlock { lang, code } => {
            let class = lang
                .as_deref()
                .map(|l| format!(" class=\"language-{}\"", l))
                .unwrap_or_default();
            out.push_str(&format!("<pre><code{}>{}</code></pre>\n",
                class, escape_html(code)));
        }
        Block::BlockQuote(blocks) => {
            out.push_str("<blockquote>\n");
            for b in blocks { render_block(b, out); }
            out.push_str("</blockquote>\n");
        }
        Block::List { ordered, start, items, .. } => {
            let tag = if *ordered {
                if *start != 1 {
                    format!("<ol start=\"{}\">", start)
                } else {
                    "<ol>".to_string()
                }
            } else {
                "<ul>".to_string()
            };
            let close = if *ordered { "</ol>" } else { "</ul>" };
            out.push_str(&tag);
            out.push('\n');
            for item in items {
                out.push_str("<li>");
                // Unwrap single para items (tight lists)
                if item.len() == 1 {
                    if let Block::Para(inlines) = &item[0] {
                        render_inlines(inlines, out);
                        out.push_str("</li>\n");
                        continue;
                    }
                }
                out.push('\n');
                for b in item { render_block(b, out); }
                out.push_str("</li>\n");
            }
            out.push_str(close);
            out.push('\n');
        }
        Block::Table { head, rows } => {
            out.push_str("<table>\n");
            if !head.is_empty() {
                out.push_str("<thead><tr>");
                for cell in head {
                    out.push_str("<th>");
                    render_inlines(cell, out);
                    out.push_str("</th>");
                }
                out.push_str("</tr></thead>\n");
            }
            out.push_str("<tbody>\n");
            for row in rows {
                out.push_str("<tr>");
                for cell in row {
                    out.push_str("<td>");
                    render_inlines(cell, out);
                    out.push_str("</td>");
                }
                out.push_str("</tr>\n");
            }
            out.push_str("</tbody></table>\n");
        }
        Block::HorizontalRule => out.push_str("<hr>\n"),
        Block::RawBlock { format, content } if format == "html" => out.push_str(content),
        Block::RawBlock { content, .. } => out.push_str(&format!("<!-- {} -->\n", content)),
    }
}

fn render_inlines(inlines: &[Inline], out: &mut String) {
    for il in inlines {
        render_inline(il, out);
    }
}

fn render_inline(il: &Inline, out: &mut String) {
    match il {
        Inline::Text(t) => out.push_str(&escape_html(t)),
        Inline::Emph(inner) => {
            out.push_str("<em>");
            render_inlines(inner, out);
            out.push_str("</em>");
        }
        Inline::Strong(inner) => {
            out.push_str("<strong>");
            render_inlines(inner, out);
            out.push_str("</strong>");
        }
        Inline::Strikethrough(inner) => {
            out.push_str("<s>");
            render_inlines(inner, out);
            out.push_str("</s>");
        }
        Inline::Code(s) => {
            out.push_str("<code>");
            out.push_str(&escape_html(s));
            out.push_str("</code>");
        }
        Inline::Link { url, title, content } => {
            if title.is_empty() {
                out.push_str(&format!("<a href=\"{}\">", escape_attr(url)));
            } else {
                out.push_str(&format!("<a href=\"{}\" title=\"{}\">",
                    escape_attr(url), escape_html(title)));
            }
            render_inlines(content, out);
            out.push_str("</a>");
        }
        Inline::Image { src, alt } => {
            let mut alt_text = String::new();
            render_inlines_plain(alt, &mut alt_text);
            out.push_str(&format!("<img src=\"{}\" alt=\"{}\">",
                escape_attr(src), escape_html(&alt_text)));
        }
        Inline::LineBreak => out.push_str("<br>\n"),
        Inline::SoftBreak => out.push('\n'),
        Inline::RawInline { format, content } if format == "html" => out.push_str(content),
        Inline::RawInline { .. } => {}
    }
}

fn render_inlines_plain(inlines: &[Inline], out: &mut String) {
    for il in inlines {
        crate::ir::doc::inline_to_text(il, out);
    }
}

pub fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
}
