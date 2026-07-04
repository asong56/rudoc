use crate::ir::doc::{Block, DocIR};

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
        SlideIR { title, slides }
    }
}
