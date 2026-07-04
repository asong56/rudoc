use indexmap::IndexMap;

/// Hierarchical tree IR. Used by: xml, opml, json.
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub tag: String,
    pub attrs: IndexMap<String, String>,
    pub children: Vec<Child>,
}

#[derive(Debug, Clone)]
pub enum Child {
    Node(TreeNode),
    Text(String),
}

impl TreeNode {
    pub fn new(tag: impl Into<String>) -> Self {
        TreeNode {
            tag: tag.into(),
            attrs: IndexMap::new(),
            children: Vec::new(),
        }
    }

    pub fn with_attr(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.attrs.insert(k.into(), v.into());
        self
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.children.push(Child::Text(text.into()));
        self
    }

    pub fn push_child(&mut self, child: TreeNode) {
        self.children.push(Child::Node(child));
    }

    /// Returns true if this node has only text content (no child elements).
    pub fn is_leaf(&self) -> bool {
        self.children
            .iter()
            .all(|c| matches!(c, Child::Text(_)))
    }

    /// Collect all text content of this node.
    pub fn text_content(&self) -> String {
        self.children
            .iter()
            .filter_map(|c| match c {
                Child::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }
}
