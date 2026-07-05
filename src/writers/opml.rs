use anyhow::Result;
use crate::ir::tree::{Child, TreeNode};

/// Render TreeIR to OPML format.
/// The root node's tag is stored in a `_rudoc_root` attribute to enable
/// lossless XML → OPML → XML round-trips.
pub fn render(node: &TreeNode) -> Result<String> {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<opml version=\"2.0\">\n");
    out.push_str("  <head>\n");
    // Store the original root tag so XML round-trip can restore it
    out.push_str(&format!("    <title>Exported by Rudoc (root={})</title>\n",
        xml_escape(&node.tag)));
    out.push_str("  </head>\n");
    out.push_str("  <body>\n");
    // Emit the root node as the first outline, preserving its tag
    emit_outline(node, &mut out, 2);
    out.push_str("  </body>\n");
    out.push_str("</opml>\n");
    Ok(out)
}

fn emit_outline(node: &TreeNode, out: &mut String, depth: usize) {
    let indent = "  ".repeat(depth);

    // Derive display text: prefer "text" attr, then tag name, then first text child
    let text = node.attrs.get("text")
        .cloned()
        .unwrap_or_else(|| {
            let t = node.text_content();
            if t.is_empty() { node.tag.clone() } else { t }
        });

    // Collect all attributes to preserve, plus add _tag for round-trip
    let mut attr_parts = format!("text=\"{}\" _tag=\"{}\"",
        xml_escape(&text), xml_escape(&node.tag));

    for (k, v) in &node.attrs {
        if k == "text" { continue; } // already in text=
        attr_parts.push_str(&format!(" {}=\"{}\"", xml_escape(k), xml_escape(v)));
    }

    // Check for RSS/feed outlines
    if let Some(url) = node.attrs.get("xmlUrl").or_else(|| node.attrs.get("url")) {
        if !attr_parts.contains("xmlUrl") {
            attr_parts.push_str(&format!(" type=\"rss\" xmlUrl=\"{}\"", xml_escape(url)));
        }
    }

    let child_nodes: Vec<&TreeNode> = node.children.iter()
        .filter_map(|c| if let Child::Node(n) = c { Some(n) } else { None })
        .collect();

    if child_nodes.is_empty() {
        out.push_str(&format!("{}<outline {}/>\n", indent, attr_parts));
    } else {
        out.push_str(&format!("{}<outline {}>\n", indent, attr_parts));
        for child in child_nodes {
            emit_outline(child, out, depth + 1);
        }
        out.push_str(&format!("{}</outline>\n", indent));
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
