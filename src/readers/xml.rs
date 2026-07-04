use anyhow::Result;
use indexmap::IndexMap;
use quick_xml::events::Event as XmlEvent;
use quick_xml::Reader;

use crate::ir::tree::{Child, TreeNode};

pub fn parse(src: &str) -> Result<TreeNode> {
    let mut reader = Reader::from_str(src);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut stack: Vec<TreeNode> = Vec::new();
    let mut root: Option<TreeNode> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Start(ref e)) => {
                let tag = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("element")
                    .to_string();
                let mut node = TreeNode::new(tag);
                for attr in e.attributes().flatten() {
                    let key = std::str::from_utf8(attr.key.as_ref())
                        .unwrap_or("")
                        .to_string();
                    let val = attr.unescape_value()
                        .map(|v| v.to_string())
                        .unwrap_or_default();
                    node.attrs.insert(key, val);
                }
                stack.push(node);
            }
            Ok(XmlEvent::End(_)) => {
                if let Some(finished) = stack.pop() {
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(Child::Node(finished));
                    } else {
                        root = Some(finished);
                    }
                }
            }
            Ok(XmlEvent::Empty(ref e)) => {
                let tag = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("element")
                    .to_string();
                let mut node = TreeNode::new(tag);
                for attr in e.attributes().flatten() {
                    let key = std::str::from_utf8(attr.key.as_ref())
                        .unwrap_or("")
                        .to_string();
                    let val = attr.unescape_value()
                        .map(|v| v.to_string())
                        .unwrap_or_default();
                    node.attrs.insert(key, val);
                }
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(Child::Node(node));
                } else {
                    root = Some(node);
                }
            }
            Ok(XmlEvent::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if !text.trim().is_empty() {
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(Child::Text(text));
                    }
                }
            }
            Ok(XmlEvent::Eof) => break,
            Err(e) => {
                // Try to recover with a synthetic root
                if let Some(node) = stack.pop() {
                    root = Some(node);
                }
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    root.ok_or_else(|| anyhow::anyhow!("No root element found in XML"))
}
