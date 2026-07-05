use anyhow::Result;
use serde_json::Value;
use crate::ir::tree::{Child, TreeNode};

pub fn parse(src: &str) -> Result<TreeNode> {
    let value: Value = serde_json::from_str(src)?;
    Ok(value_to_node("root", &value))
}

fn value_to_node(tag: &str, value: &Value) -> TreeNode {
    match value {
        Value::Object(map) => {
            let mut node = TreeNode::new(tag);
            for (k, v) in map {
                node.children.push(Child::Node(value_to_node(k, v)));
            }
            node
        }
        Value::Array(arr) => {
            let mut node = TreeNode::new(tag);
            // Use "item" as the repeated tag for array elements
            let child_tag = format!("{}_item", tag.trim_end_matches('s'));
            for item in arr {
                node.children.push(Child::Node(value_to_node(&child_tag, item)));
            }
            node
        }
        Value::String(s) => {
            TreeNode::new(tag).with_text(s.clone())
        }
        Value::Number(n) => {
            TreeNode::new(tag).with_text(n.to_string())
        }
        Value::Bool(b) => {
            TreeNode::new(tag).with_text(b.to_string())
        }
        Value::Null => {
            TreeNode::new(tag).with_attr("null", "true")
        }
    }
}
