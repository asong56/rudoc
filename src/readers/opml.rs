use anyhow::Result;
use crate::ir::tree::{Child, TreeNode};

/// Parse OPML into TreeIR.
/// Restores original XML tag names from the `_tag` attribute when present
/// (written by our OPML writer), enabling lossless XML → OPML → XML round-trips.
pub fn parse(src: &str) -> Result<TreeNode> {
    let raw = crate::readers::xml::parse(src)?;
    normalize_opml(raw)
}

fn normalize_opml(root: TreeNode) -> Result<TreeNode> {
    // Extract root tag from <title> if written by Rudoc
    let mut original_root_tag: Option<String> = None;
    for child in &root.children {
        if let Child::Node(n) = child {
            if n.tag.eq_ignore_ascii_case("head") {
                for hc in &n.children {
                    if let Child::Node(hn) = hc {
                        if hn.tag.eq_ignore_ascii_case("title") {
                            let title = hn.text_content();
                            // Format: "Exported by Rudoc (root=TAGNAME)"
                            if let Some(start) = title.find("(root=") {
                                let rest = &title[start + 6..];
                                if let Some(end) = rest.find(')') {
                                    original_root_tag = Some(rest[..end].to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Find <body>
    for child in &root.children {
        if let Child::Node(n) = child {
            if n.tag.eq_ignore_ascii_case("body") {
                let root_tag = original_root_tag.unwrap_or_else(|| "root".to_string());
                return Ok(normalize_body(n.clone(), &root_tag));
            }
        }
    }
    Ok(root)
}

fn normalize_body(body: TreeNode, root_tag: &str) -> TreeNode {
    // If the body has exactly one child <outline _tag="root_tag">, unwrap it
    let outline_children: Vec<&TreeNode> = body.children.iter()
        .filter_map(|c| if let Child::Node(n) = c { Some(n) } else { None })
        .collect();

    if outline_children.len() == 1 {
        let only = outline_children[0];
        // Check if this is our preserved root
        let has_root_tag = only.attrs.get("_tag")
            .map(|t| t == root_tag)
            .unwrap_or(false);
        if has_root_tag {
            return normalize_outline(only.clone());
        }
    }

    // Multiple top-level outlines: wrap in a generic root
    let mut wrapper = TreeNode::new(root_tag);
    for child in body.children {
        if let Child::Node(n) = child {
            wrapper.children.push(Child::Node(normalize_outline(n)));
        }
    }
    wrapper
}

fn normalize_outline(outline: TreeNode) -> TreeNode {
    // Restore original tag from _tag attribute, or use text attr, or keep tag
    let restored_tag = outline.attrs.get("_tag")
        .cloned()
        .unwrap_or_else(|| {
            outline.attrs.get("text")
                .cloned()
                .unwrap_or_else(|| outline.tag.clone())
        });

    let mut node = TreeNode::new(restored_tag);

    // Copy all attributes except _tag and text
    for (k, v) in &outline.attrs {
        if k == "_tag" || k == "text" { continue; }
        node.attrs.insert(k.clone(), v.clone());
    }

    // Check if this was a leaf text node: no child outline elements
    let child_outlines: Vec<_> = outline.children.iter()
        .filter(|c| matches!(c, Child::Node(_)))
        .collect();

    if child_outlines.is_empty() {
        // Restore the original text content from the "text" attribute
        // (only if the text differs from the tag name, i.e. it was real content)
        if let Some(text_content) = outline.attrs.get("text") {
            if text_content != &node.tag {
                node.children.push(Child::Text(text_content.clone()));
            }
        }
    } else {
        // Recurse into child outlines
        for child in outline.children {
            if let Child::Node(child_outline) = child {
                node.children.push(Child::Node(normalize_outline(child_outline)));
            }
        }
    }
    node
}
