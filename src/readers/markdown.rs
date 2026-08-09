use anyhow::Result;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag};

use crate::ir::doc::{Block, DocIR, Inline};

pub fn parse(src: &str) -> Result<DocIR> {
    let (frontmatter, body) = split_frontmatter(src);

    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_SMART_PUNCTUATION);

    let events: Vec<Event> = Parser::new_ext(body, opts).collect();
    let mut doc = DocIR::new();
    let mut ctx = ParseCtx::default();

    for event in events {
        process_event(event, &mut ctx, &mut doc);
    }

    if let Some(fm) = frontmatter {
        apply_frontmatter(&fm, &mut doc);
    }

    Ok(doc)
}

/// Split a leading YAML frontmatter block (`---\n ... \n---`) off the
/// source. Without this, the frontmatter lines are fed to the Markdown
/// parser itself, which mangles them into paragraphs/rules instead of
/// metadata. Returns (frontmatter_text, remaining_body).
fn split_frontmatter(src: &str) -> (Option<String>, &str) {
    let stripped = src.strip_prefix('\u{feff}').unwrap_or(src);
    let leading_ws_len = stripped.len() - stripped.trim_start().len();
    let after_ws = &stripped[leading_ws_len..];

    if !after_ws.starts_with("---") { return (None, src); }
    // The delimiter line must be exactly "---" (optionally trailing spaces).
    let rest = &after_ws[3..];
    let rest = match rest.split_once('\n') {
        Some((first_line_rest, tail)) if first_line_rest.trim().is_empty() => tail,
        _ => return (None, src),
    };

    // Find the closing "---" or "..." delimiter on its own line.
    let mut byte_off = 0usize;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches('\n').trim();
        if trimmed == "---" || trimmed == "..." {
            let fm_text = &rest[..byte_off];
            let body_start_in_rest = byte_off + line.len();
            let body_start = (after_ws.len() - rest.len()) + body_start_in_rest;
            let body = &after_ws[body_start..];
            return (Some(fm_text.to_string()), body);
        }
        byte_off += line.len();
    }
    (None, src)
}

/// Minimal, dependency-free YAML scalar parser for `key: value` frontmatter.
/// Handles the common Pandoc-style keys plus arbitrary custom keys (kept as
/// raw text via a `RawBlock` so round-tripping through Markdown preserves
/// them even though the doc IR has no generic metadata bag).
fn apply_frontmatter(fm: &str, doc: &mut DocIR) {
    let mut extra: Vec<(String, String)> = Vec::new();
    for line in fm.lines() {
        let line = line.trim_end();
        if line.trim().is_empty() { continue; }
        let Some((key, val)) = line.split_once(':') else { continue };
        let key = key.trim();
        // Skip nested/list entries (indented or starting with '-'); this is
        // a flat-scalar parser only.
        if key.is_empty() || line.starts_with(' ') || line.starts_with('-') { continue; }
        let val = unquote_yaml_scalar(val.trim());
        match key {
            "title" => doc.metadata.title = Some(val),
            "author" => doc.metadata.author = Some(val),
            "date" => doc.metadata.date = Some(val),
            "lang" | "language" => doc.metadata.lang = Some(val),
            _ => extra.push((key.to_string(), val)),
        }
    }
    if !extra.is_empty() {
        // Preserve unrecognised keys as a raw YAML block so a md→md
        // round-trip doesn't silently drop them.
        let mut content = String::from("---\n");
        for (k, v) in &extra {
            content.push_str(&format!("{}: \"{}\"\n", k, v.replace('"', "\\\"")));
        }
        content.push_str("---");
        doc.blocks.insert(0, Block::RawBlock { format: "yaml".into(), content });
    }
}

fn unquote_yaml_scalar(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
        || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2)
    {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

#[derive(Default)]
struct ParseCtx {
    inline_stack: Vec<Vec<Inline>>,
    block_stack:  Vec<BlockFrame>,
    code_lang:    Option<String>,
    // Table state
    in_table_head:      bool,
    table_head:         Vec<Vec<Inline>>,
    table_rows:         Vec<Vec<Vec<Inline>>>,
    table_current_row:  Vec<Vec<Inline>>,
}

enum BlockFrame {
    BlockQuote(Vec<Block>),
    ListItem(Vec<Block>),
    List { ordered: bool, start: u64, items: Vec<Vec<Block>> },
}

impl ParseCtx {
    fn push_inline_ctx(&mut self) { self.inline_stack.push(Vec::new()); }
    fn pop_inline_ctx(&mut self) -> Vec<Inline> { self.inline_stack.pop().unwrap_or_default() }

    fn push_inline(&mut self, il: Inline) {
        if let Some(top) = self.inline_stack.last_mut() { top.push(il); }
    }

    /// Where to push a finished block: innermost list-item or blockquote, else doc root.
    fn push_block(&mut self, block: Block, doc: &mut DocIR) {
        for frame in self.block_stack.iter_mut().rev() {
            match frame {
                BlockFrame::ListItem(blocks) => { blocks.push(block); return; }
                BlockFrame::BlockQuote(blocks) => { blocks.push(block); return; }
                _ => {}
            }
        }
        doc.blocks.push(block);
    }
}

fn level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1, HeadingLevel::H2 => 2, HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4, HeadingLevel::H5 => 5, HeadingLevel::H6 => 6,
    }
}

fn process_event(event: Event<'_>, ctx: &mut ParseCtx, doc: &mut DocIR) {
    match event {
        // ── Block containers ────────────────────────────────────────────
        Event::Start(Tag::Paragraph) => ctx.push_inline_ctx(),
        Event::End(Tag::Paragraph) => {
            let inlines = ctx.pop_inline_ctx();
            if !inlines.is_empty() {
                ctx.push_block(Block::Para(inlines), doc);
            }
        }

        Event::Start(Tag::Heading(_level, ..)) => ctx.push_inline_ctx(),
        Event::End(Tag::Heading(level, ..)) => {
            let inlines = ctx.pop_inline_ctx();
            ctx.push_block(Block::Heading(level_to_u8(level), inlines), doc);
        }

        Event::Start(Tag::BlockQuote) => {
            ctx.block_stack.push(BlockFrame::BlockQuote(Vec::new()));
        }
        Event::End(Tag::BlockQuote) => {
            if let Some(BlockFrame::BlockQuote(blocks)) = ctx.block_stack.pop() {
                ctx.push_block(Block::BlockQuote(blocks), doc);
            }
        }

        Event::Start(Tag::List(start_num)) => {
            ctx.block_stack.push(BlockFrame::List {
                ordered: start_num.is_some(),
                start:   start_num.unwrap_or(1),
                items:   Vec::new(),
            });
        }
        Event::End(Tag::List(_)) => {
            if let Some(BlockFrame::List { ordered, start, items }) = ctx.block_stack.pop() {
                ctx.push_block(Block::List { ordered, start, tight: true, items }, doc);
            }
        }

        // ── List item: push an inline ctx for tight-list text ────────────
        Event::Start(Tag::Item) => {
            ctx.block_stack.push(BlockFrame::ListItem(Vec::new()));
            ctx.push_inline_ctx(); // catch bare Text in tight lists
        }
        Event::End(Tag::Item) => {
            // Flush any loose text collected at item level
            let loose_text = ctx.pop_inline_ctx();
            if let Some(BlockFrame::ListItem(ref mut blocks)) = ctx.block_stack.last_mut() {
                if !loose_text.is_empty() && blocks.is_empty() {
                    blocks.push(Block::Para(loose_text));
                }
            }
            if let Some(BlockFrame::ListItem(blocks)) = ctx.block_stack.pop() {
                for frame in ctx.block_stack.iter_mut().rev() {
                    if let BlockFrame::List { items, .. } = frame {
                        items.push(blocks);
                        break;
                    }
                }
            }
        }

        // ── Code blocks ─────────────────────────────────────────────────
        Event::Start(Tag::CodeBlock(kind)) => {
            use pulldown_cmark::CodeBlockKind;
            ctx.code_lang = match kind {
                CodeBlockKind::Fenced(info) => {
                    let s = info.to_string();
                    let l = s.split(|c: char| c == ' ' || c == ',').next().unwrap_or("");
                    if l.is_empty() { None } else { Some(l.to_string()) }
                }
                CodeBlockKind::Indented => None,
            };
            ctx.push_inline_ctx();
        }
        Event::End(Tag::CodeBlock(_)) => {
            let inlines = ctx.pop_inline_ctx();
            let code: String = inlines.iter().filter_map(|i| if let Inline::Text(t) = i { Some(t.as_str()) } else { None }).collect();
            let lang = ctx.code_lang.take();
            ctx.push_block(Block::CodeBlock { lang, code }, doc);
        }

        // ── Table ────────────────────────────────────────────────────────
        Event::Start(Tag::Table(_)) => {
            ctx.table_head.clear();
            ctx.table_rows.clear();
            ctx.table_current_row.clear();
            ctx.in_table_head = false;
        }
        Event::End(Tag::Table(_)) => {
            let block = Block::Table {
                head: std::mem::take(&mut ctx.table_head),
                rows: std::mem::take(&mut ctx.table_rows),
            };
            ctx.push_block(block, doc);
        }
        Event::Start(Tag::TableHead) => { ctx.in_table_head = true; ctx.table_current_row.clear(); }
        Event::End(Tag::TableHead) => { ctx.table_head = std::mem::take(&mut ctx.table_current_row); ctx.in_table_head = false; }
        Event::Start(Tag::TableRow) => { ctx.table_current_row.clear(); }
        Event::End(Tag::TableRow) => {
            let row = std::mem::take(&mut ctx.table_current_row);
            ctx.table_rows.push(row);
        }
        Event::Start(Tag::TableCell) => { ctx.push_inline_ctx(); }
        Event::End(Tag::TableCell) => {
            let cell = ctx.pop_inline_ctx();
            ctx.table_current_row.push(cell);
        }

        // ── Inline formatting ────────────────────────────────────────────
        Event::Start(Tag::Emphasis)      => ctx.push_inline_ctx(),
        Event::End(Tag::Emphasis) => {
            let inner = ctx.pop_inline_ctx();
            ctx.push_inline(Inline::Emph(inner));
        }
        Event::Start(Tag::Strong)        => ctx.push_inline_ctx(),
        Event::End(Tag::Strong) => {
            let inner = ctx.pop_inline_ctx();
            ctx.push_inline(Inline::Strong(inner));
        }
        Event::Start(Tag::Strikethrough) => ctx.push_inline_ctx(),
        Event::End(Tag::Strikethrough) => {
            let inner = ctx.pop_inline_ctx();
            ctx.push_inline(Inline::Strikethrough(inner));
        }
        Event::Start(Tag::Link(_, url, title)) => {
            ctx.push_inline_ctx();
            // stash url/title as a sentinel
            ctx.inline_stack.last_mut().unwrap().push(Inline::RawInline {
                format: "__link__".into(),
                content: format!("{}|||{}", url, title),
            });
        }
        Event::End(Tag::Link(_, url, title)) => {
            let mut inner = ctx.pop_inline_ctx();
            // remove sentinel
            if inner.first().map(|i| matches!(i, Inline::RawInline { format, .. } if format == "__link__")).unwrap_or(false) {
                inner.remove(0);
            }
            ctx.push_inline(Inline::Link { url: url.to_string(), title: title.to_string(), content: inner });
        }
        Event::Start(Tag::Image(_, _src, _)) => ctx.push_inline_ctx(),
        Event::End(Tag::Image(_, src, _)) => {
            let alt = ctx.pop_inline_ctx();
            ctx.push_inline(Inline::Image { src: src.to_string(), alt });
        }

        // ── Leaf events ──────────────────────────────────────────────────
        Event::Text(t)     => ctx.push_inline(Inline::Text(t.to_string())),
        Event::Code(t)     => ctx.push_inline(Inline::Code(t.to_string())),
        Event::SoftBreak   => ctx.push_inline(Inline::SoftBreak),
        Event::HardBreak   => ctx.push_inline(Inline::LineBreak),
        Event::Html(h)     => ctx.push_inline(Inline::RawInline { format: "html".into(), content: h.to_string() }),
        Event::Rule        => ctx.push_block(Block::HorizontalRule, doc),
        // GFM task list checkbox ("- [ ] foo" / "- [x] foo"). pulldown-cmark
        // emits this as the first event inside the Item's inline run; render
        // it as a literal "[x] "/"[ ] " prefix so it survives into every
        // output format without needing a dedicated IR variant.
        Event::TaskListMarker(checked) => {
            ctx.push_inline(Inline::Text(if checked { "[x] ".into() } else { "[ ] ".into() }));
        }
        _                  => {}
    }
}

pub fn extract_title(src: &str) -> Option<String> {
    let src = src.trim_start();
    if !src.starts_with("---") { return None; }
    let rest = src.trim_start_matches('-').trim_start_matches('\n');
    for line in rest.lines() {
        if line.starts_with("title:") {
            return Some(line.trim_start_matches("title:").trim().trim_matches('"').trim_matches('\'').to_string());
        }
        if line == "---" { break; }
    }
    None
}
