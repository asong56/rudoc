use anyhow::Result;
use crate::ir::tree::{Child, TreeNode};

pub fn render(node: &TreeNode) -> Result<String> {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<opml version=\"2.0\">\n");
    out.push_str("  <head>\n");
    out.push_str("    <title>Exported by Rudoc</title>\n");
    out.push_str("  </head>\n");
    out.push_str("  <body>\n");
    write_outlines(node, &mut out, 2);
    out.push_str("  </body>\n");
    out.push_str("</opml>\n");
    Ok(out)
}

fn write_outlines(node: &TreeNode, out: &mut String, depth: usize) {
    let indent = "  ".repeat(depth);
    for child in &node.children {
        if let Child::Node(n) = child {
            let text = n.attrs.get("text")
                .cloned()
                .unwrap_or_else(|| n.tag.clone());
            let escaped = xml_escape(&text);

            if n.children.is_empty() {
                // Check if there's a url attribute
                if let Some(url) = n.attrs.get("xmlUrl").or_else(|| n.attrs.get("url")) {
                    out.push_str(&format!(
                        "{}<outline text=\"{}\" type=\"rss\" xmlUrl=\"{}\"/>\n",
                        indent, escaped, xml_escape(url)
                    ));
                } else {
                    out.push_str(&format!("{}<outline text=\"{}\"/>\n", indent, escaped));
                }
            } else {
                out.push_str(&format!("{}<outline text=\"{}\">\n", indent, escaped));
                write_outlines(n, out, depth + 1);
                out.push_str(&format!("{}</outline>\n", indent));
            }
        }
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
