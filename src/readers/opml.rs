use anyhow::Result;
use crate::ir::tree::{Child, TreeNode};

/// Parse OPML into TreeIR. OPML is valid XML; we use the XML reader then
/// normalize the tree so outline nodes become generic tree nodes.
pub fn parse(src: &str) -> Result<TreeNode> {
    let raw = crate::readers::xml::parse(src)?;
    // Normalize: find <body> and return it as the root, with outlines as children
    normalize_opml(raw)
}

fn normalize_opml(root: TreeNode) -> Result<TreeNode> {
    // root should be <opml>; find <body> inside it
    for child in &root.children {
        if let Child::Node(n) = child {
            if n.tag.eq_ignore_ascii_case("body") {
                return Ok(normalize_body(n.clone()));
            }
        }
    }
    // If no body found, just return root
    Ok(root)
}

fn normalize_body(body: TreeNode) -> TreeNode {
    // Each <outline> becomes a TreeNode with tag = text attr
    let mut result = TreeNode::new("outlines");
    for child in body.children {
        if let Child::Node(outline) = child {
            result.children.push(Child::Node(normalize_outline(outline)));
        }
    }
    result
}

fn normalize_outline(outline: TreeNode) -> TreeNode {
    let tag = outline.attrs
        .get("text")
        .cloned()
        .unwrap_or_else(|| outline.tag.clone());
    let mut node = TreeNode::new(tag);
    // Preserve all original attributes
    node.attrs = outline.attrs;
    // Recurse into children
    for child in outline.children {
        if let Child::Node(child_outline) = child {
            node.children.push(Child::Node(normalize_outline(child_outline)));
        }
    }
    node
}
