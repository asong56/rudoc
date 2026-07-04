use anyhow::Result;
use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use std::io::Cursor;

use crate::ir::tree::{Child, TreeNode};

pub fn render(node: &TreeNode, pretty: bool) -> Result<String> {
    let buf = Vec::new();
    let cursor = Cursor::new(buf);
    let mut writer = if pretty {
        Writer::new_with_indent(cursor, b' ', 2)
    } else {
        Writer::new(cursor)
    };

    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    write_node(&mut writer, node)?;

    let bytes = writer.into_inner().into_inner();
    Ok(String::from_utf8(bytes)?)
}

fn write_node(writer: &mut Writer<Cursor<Vec<u8>>>, node: &TreeNode) -> Result<()> {
    let tag_name = sanitize_tag(&node.tag);

    if node.children.is_empty() {
        let mut elem = BytesStart::new(tag_name.as_str());
        for (k, v) in &node.attrs {
            let key = sanitize_tag(k);
            elem.push_attribute((key.as_str(), v.as_str()));
        }
        writer.write_event(Event::Empty(elem))?;
        return Ok(());
    }

    let mut start = BytesStart::new(tag_name.as_str());
    for (k, v) in &node.attrs {
        let key = sanitize_tag(k);
        start.push_attribute((key.as_str(), v.as_str()));
    }
    writer.write_event(Event::Start(start))?;

    for child in &node.children {
        match child {
            Child::Node(n) => write_node(writer, n)?,
            Child::Text(t) => {
                writer.write_event(Event::Text(BytesText::new(t.as_str())))?;
            }
        }
    }

    writer.write_event(Event::End(BytesEnd::new(tag_name.as_str())))?;
    Ok(())
}

fn sanitize_tag(s: &str) -> String {
    if s.is_empty() { return "element".to_string(); }
    let mut out = String::new();
    let first = s.chars().next().unwrap();
    if first.is_alphabetic() || first == '_' { out.push(first); }
    else { out.push('_'); out.push(first); }
    for c in s.chars().skip(1) {
        if c.is_alphanumeric() || c == '_' || c == '-' || c == '.' { out.push(c); }
        else { out.push('_'); }
    }
    out
}
