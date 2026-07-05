use anyhow::Result;
use serde_json::{Map, Value};
use crate::ir::tree::{Child, TreeNode};

pub fn render(node: &TreeNode, pretty: bool) -> Result<String> {
    let value = node_to_value(node);
    let out = if pretty {
        serde_json::to_string_pretty(&value)?
    } else {
        serde_json::to_string(&value)?
    };
    Ok(out)
}

fn node_to_value(node: &TreeNode) -> Value {
    // Leaf node with only text children
    if node.is_leaf() {
        let text = node.text_content();
        // Try to parse as number or bool
        if let Ok(n) = text.parse::<f64>() {
            return Value::Number(serde_json::Number::from_f64(n)
                .unwrap_or_else(|| serde_json::Number::from(0)));
        }
        match text.to_lowercase().as_str() {
            "true" => return Value::Bool(true),
            "false" => return Value::Bool(false),
            _ => {}
        }
        if node.attrs.get("null").map(|v| v == "true").unwrap_or(false) {
            return Value::Null;
        }
        return Value::String(text);
    }

    // Node with children → object
    let mut map = Map::new();

    // Include attributes as fields with "@" prefix (XML convention)
    for (k, v) in &node.attrs {
        if k != "null" {
            map.insert(format!("@{}", k), Value::String(v.clone()));
        }
    }

    // Group children by tag name
    let mut tag_groups: indexmap::IndexMap<String, Vec<&TreeNode>> = indexmap::IndexMap::new();
    for child in &node.children {
        if let Child::Node(n) = child {
            tag_groups.entry(n.tag.clone()).or_default().push(n);
        }
    }

    for (tag, nodes) in tag_groups {
        if nodes.len() == 1 {
            map.insert(tag, node_to_value(nodes[0]));
        } else {
            let arr: Vec<Value> = nodes.iter().map(|n| node_to_value(n)).collect();
            map.insert(tag, Value::Array(arr));
        }
    }

    Value::Object(map)
}
