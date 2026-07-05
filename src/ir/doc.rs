/// Rich-document intermediate representation.
/// Used by: md, html, txt, docx, typ, pdf
#[derive(Debug, Clone, Default)]
pub struct DocIR {
    pub metadata: Metadata,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Default)]
pub struct Metadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub date: Option<String>,
    pub lang: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Block {
    Heading(u8, Vec<Inline>),
    Para(Vec<Inline>),
    CodeBlock {
        lang: Option<String>,
        code: String,
    },
    BlockQuote(Vec<Block>),
    List {
        ordered: bool,
        start: u64,
        tight: bool,
        items: Vec<Vec<Block>>,
    },
    Table {
        head: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    HorizontalRule,
    #[allow(dead_code)]
    RawBlock {
        format: String,
        content: String,
    },
}

#[derive(Debug, Clone)]
pub enum Inline {
    Text(String),
    Emph(Vec<Inline>),
    Strong(Vec<Inline>),
    Strikethrough(Vec<Inline>),
    Code(String),
    Link {
        url: String,
        title: String,
        content: Vec<Inline>,
    },
    Image {
        src: String,
        alt: Vec<Inline>,
    },
    LineBreak,
    SoftBreak,
    RawInline {
        format: String,
        content: String,
    },
}

impl DocIR {
    pub fn new() -> Self {
        Self::default()
    }

    /// Flatten all inlines in the document to a plain string (for text extraction).
    pub fn plain_text(&self) -> String {
        let mut out = String::new();
        for block in &self.blocks {
            block_to_text(block, &mut out);
        }
        out
    }
}

fn block_to_text(block: &Block, out: &mut String) {
    match block {
        Block::Heading(_, inlines) | Block::Para(inlines) => {
            for il in inlines {
                inline_to_text(il, out);
            }
            out.push('\n');
        }
        Block::CodeBlock { code, .. } => {
            out.push_str(code);
            out.push('\n');
        }
        Block::BlockQuote(blocks) => {
            for b in blocks {
                block_to_text(b, out);
            }
        }
        Block::List { items, .. } => {
            for item in items {
                for b in item {
                    block_to_text(b, out);
                }
            }
        }
        Block::Table { head, rows } => {
            for cell in head {
                for il in cell {
                    inline_to_text(il, out);
                }
                out.push('\t');
            }
            out.push('\n');
            for row in rows {
                for cell in row {
                    for il in cell {
                        inline_to_text(il, out);
                    }
                    out.push('\t');
                }
                out.push('\n');
            }
        }
        Block::HorizontalRule => out.push_str("---\n"),
        Block::RawBlock { content, .. } => out.push_str(content),
    }
}

pub fn inline_to_text(il: &Inline, out: &mut String) {
    match il {
        Inline::Text(s) => out.push_str(s),
        Inline::Emph(inner) | Inline::Strong(inner) | Inline::Strikethrough(inner) => {
            for i in inner {
                inline_to_text(i, out);
            }
        }
        Inline::Code(s) => out.push_str(s),
        Inline::Link { content, .. } => {
            for i in content {
                inline_to_text(i, out);
            }
        }
        Inline::Image { alt, .. } => {
            for i in alt {
                inline_to_text(i, out);
            }
        }
        Inline::LineBreak | Inline::SoftBreak => out.push('\n'),
        Inline::RawInline { content, .. } => out.push_str(content),
    }
}

/// Public wrapper around block_to_text for use by other modules.
pub fn block_to_text_pub(block: &Block, out: &mut String) {
    block_to_text(block, out);
}
