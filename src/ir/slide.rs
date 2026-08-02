use crate::ir::doc::{Block, DocIR, Inline};

/// Slide-oriented IR derived from DocIR.
#[derive(Debug, Clone, Default)]
pub struct SlideIR {
    pub title: String,
    pub slides: Vec<Slide>,
}

#[derive(Debug, Clone, Default)]
pub struct Slide {
    pub title: String,
    pub body: Vec<Block>,
    pub notes: Option<String>,
}

impl SlideIR {
    /// Convert a DocIR into slides, splitting on headings at `slide_level`.
    pub fn from_doc(doc: &DocIR, slide_level: u8) -> Self {
        let title = doc
            .metadata
            .title
            .clone()
            .unwrap_or_else(|| "Presentation".to_string());
        let mut slides: Vec<Slide> = Vec::new();
        let mut current: Option<Slide> = None;

        for block in &doc.blocks {
            match block {
                Block::Heading(level, inlines) if *level == slide_level => {
                    if let Some(prev) = current.take() {
                        slides.push(prev);
                    }
                    let mut slide_title = String::new();
                    for il in inlines {
                        crate::ir::doc::inline_to_text(il, &mut slide_title);
                    }
                    current = Some(Slide {
                        title: slide_title,
                        body: Vec::new(),
                        notes: None,
                    });
                }
                other => {
                    // Speaker notes are authored as an HTML comment:
                    // <!-- notes: some text --> (as its own block, or as a raw
                    // inline sitting alone inside a paragraph). Extract them
                    // into Slide::notes instead of rendering them as body text.
                    if let Some(note_text) = extract_notes_comment(other) {
                        let slide = current.get_or_insert_with(|| Slide {
                            title: title.clone(),
                            body: Vec::new(),
                            notes: None,
                        });
                        match &mut slide.notes {
                            Some(existing) => {
                                existing.push('\n');
                                existing.push_str(&note_text);
                            }
                            None => slide.notes = Some(note_text),
                        }
                        continue;
                    }

                    if let Some(ref mut slide) = current {
                        slide.body.push(other.clone());
                    } else {
                        // Content before first heading → title slide body
                        let intro = current.get_or_insert_with(|| Slide {
                            title: title.clone(),
                            body: Vec::new(),
                            notes: None,
                        });
                        intro.body.push(other.clone());
                    }
                }
            }
        }
        if let Some(last) = current {
            slides.push(last);
        }

        // Guarantee at least one slide so we never emit an empty <p:sldIdLst>,
        // which PowerPoint/LibreOffice treat as a corrupt package.
        if slides.is_empty() {
            slides.push(Slide {
                title: title.clone(),
                body: Vec::new(),
                notes: None,
            });
        }

        SlideIR { title, slides }
    }
}

/// Recognizes a block whose entire content is a single HTML comment of the
/// form `<!-- notes: TEXT -->` (case-insensitive on the `notes:` marker) and
/// returns TEXT. Handles both a bare `Block::RawBlock` comment and a
/// `Block::Para` that contains nothing but a single `RawInline` comment
/// (which is how pulldown-cmark surfaces a comment sitting on its own line).
fn extract_notes_comment(block: &Block) -> Option<String> {
    let raw = match block {
        Block::RawBlock { format, content } if format == "html" => Some(content.as_str()),
        Block::Para(inlines) if inlines.len() == 1 => match &inlines[0] {
            Inline::RawInline { format, content } if format == "html" => Some(content.as_str()),
            _ => None,
        },
        _ => None,
    }?;

    let trimmed = raw.trim();
    let inner = trimmed
        .strip_prefix("<!--")?
        .strip_suffix("-->")?
        .trim();

    let lower = inner.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("notes:") {
        // Re-slice the *original* (non-lowercased) string so casing in the
        // actual note text is preserved.
        let start = inner.len() - rest.len();
        Some(inner[start..].trim().to_string())
    } else {
        None
    }
}
