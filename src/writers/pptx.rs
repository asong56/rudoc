//! PPTX writer — generates a standards-compliant .pptx from SlideIR.
//!
//! Two correctness properties matter beyond just "PowerPoint can open it":
//! 1. Deterministic bytes: converting the same input twice must produce a
//!    byte-identical file, so every zip entry is stamped with a fixed
//!    timestamp instead of the wall-clock time.
//! 2. Image/document-position fidelity: if one image on a slide fails to
//!    load (bad path, corrupt file, unsupported format), every image after
//!    it must still land in its correct position rather than shifting up
//!    by one slot (see `PendingImage::occurrence` below).
use anyhow::Result;
use image::ImageEncoder;
use std::io::Write;
use std::path::Path;
use zip::{write::FileOptions, ZipWriter};

use crate::ir::doc::{Block, Inline};
use crate::ir::slide::{Slide, SlideIR};

/// One embedded picture collected while walking a slide's body.
struct PendingImage {
    /// 1-based index used in `ppt/media/imageN.<ext>` and the rel id.
    index: usize,
    /// Position of this image among *all* `Inline::Image` nodes on the
    /// slide, including ones that failed to load. Because `try_load_image`
    /// can return `None`, this slide's `Vec<PendingImage>` may have gaps
    /// relative to the document; `occurrence` is how the render pass lines
    /// each successfully-loaded image back up with its original slot
    /// instead of assuming positional (index-only) correspondence.
    occurrence: usize,
    ext: &'static str,
    bytes: Vec<u8>,
    /// Width/height in EMU, already clamped to fit inside the slide body box.
    w_emu: u32,
    h_emu: u32,
}

pub fn render(slides_in: &SlideIR) -> Result<Vec<u8>> {
    // An empty <p:sldIdLst> is treated as a corrupt package by PowerPoint/
    // LibreOffice. SlideIR::from_doc already guards against this, but we
    // defend here too in case `render` is ever called directly.
    let mut owned;
    let slides: &SlideIR = if slides_in.slides.is_empty() {
        owned = slides_in.clone();
        owned.slides.push(Slide {
            title: slides_in.title.clone(),
            body: Vec::new(),
            notes: None,
        });
        &owned
    } else {
        slides_in
    };

    let buf = Vec::new();
    let cursor = std::io::Cursor::new(buf);
    let mut zip = ZipWriter::new(cursor);
    // Fixed timestamp (the zip format's own epoch) on every entry: without
    // this, two conversions of the same input produce different bytes
    // purely from wall-clock time, which breaks anything that hashes or
    // diffs the output (reproducible builds, content-addressed caches, etc).
    let opts = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default());

    // Resolve every image up front, per slide, so we know which media
    // files/extensions exist before writing [Content_Types].xml.
    let mut images_per_slide: Vec<Vec<PendingImage>> = Vec::with_capacity(slides.slides.len());
    let mut media_index = 1usize;
    let mut any_png = false;
    let mut any_jpeg = false;
    for slide in &slides.slides {
        let mut found = Vec::new();
        let mut occurrence = 0usize;
        collect_images(&slide.body, &mut media_index, &mut occurrence, &mut found);
        for img in &found {
            match img.ext {
                "png" => any_png = true,
                _ => any_jpeg = true,
            }
        }
        images_per_slide.push(found);
    }

    let notes_flags: Vec<bool> = slides
        .slides
        .iter()
        .map(|s| s.notes.as_deref().map(|n| !n.trim().is_empty()).unwrap_or(false))
        .collect();
    let has_any_notes = notes_flags.iter().any(|b| *b);

    zip.start_file("[Content_Types].xml", opts)?;
    zip.write_all(
        content_types_xml(slides.slides.len(), &notes_flags, any_png, any_jpeg).as_bytes(),
    )?;

    zip.start_file("_rels/.rels", opts)?;
    zip.write_all(ROOT_RELS.as_bytes())?;

    zip.start_file("docProps/core.xml", opts)?;
    zip.write_all(core_props_xml(&slides.title).as_bytes())?;
    zip.start_file("docProps/app.xml", opts)?;
    zip.write_all(app_props_xml(slides.slides.len()).as_bytes())?;

    zip.start_file("ppt/presentation.xml", opts)?;
    zip.write_all(presentation_xml(slides.slides.len(), has_any_notes).as_bytes())?;

    zip.start_file("ppt/_rels/presentation.xml.rels", opts)?;
    zip.write_all(presentation_rels(slides.slides.len(), has_any_notes).as_bytes())?;

    zip.start_file("ppt/slideLayouts/slideLayout1.xml", opts)?;
    zip.write_all(SLIDE_LAYOUT.as_bytes())?;
    zip.start_file("ppt/slideLayouts/_rels/slideLayout1.xml.rels", opts)?;
    zip.write_all(SLIDE_LAYOUT_RELS.as_bytes())?;

    zip.start_file("ppt/slideMasters/slideMaster1.xml", opts)?;
    zip.write_all(SLIDE_MASTER.as_bytes())?;
    zip.start_file("ppt/slideMasters/_rels/slideMaster1.xml.rels", opts)?;
    zip.write_all(SLIDE_MASTER_RELS.as_bytes())?;

    zip.start_file("ppt/theme/theme1.xml", opts)?;
    zip.write_all(THEME.as_bytes())?;

    if has_any_notes {
        zip.start_file("ppt/notesMasters/notesMaster1.xml", opts)?;
        zip.write_all(NOTES_MASTER.as_bytes())?;
        zip.start_file("ppt/notesMasters/_rels/notesMaster1.xml.rels", opts)?;
        zip.write_all(NOTES_MASTER_RELS.as_bytes())?;
    }

    for (i, slide) in slides.slides.iter().enumerate() {
        let num = i + 1;
        let images = &images_per_slide[i];
        let has_notes = notes_flags[i];

        zip.start_file(&format!("ppt/slides/slide{}.xml", num), opts)?;
        zip.write_all(slide_xml(slide, images).as_bytes())?;

        zip.start_file(&format!("ppt/slides/_rels/slide{}.xml.rels", num), opts)?;
        zip.write_all(slide_rels(num, images, has_notes).as_bytes())?;

        for img in images {
            zip.start_file(&format!("ppt/media/image{}.{}", img.index, img.ext), opts)?;
            zip.write_all(&img.bytes)?;
        }

        if has_notes {
            zip.start_file(&format!("ppt/notesSlides/notesSlide{}.xml", num), opts)?;
            zip.write_all(notes_slide_xml(slide.notes.as_deref().unwrap_or("")).as_bytes())?;
            zip.start_file(&format!("ppt/notesSlides/_rels/notesSlide{}.xml.rels", num), opts)?;
            zip.write_all(notes_slide_rels(num).as_bytes())?;
        }
    }

    let result = zip.finish()?;
    Ok(result.into_inner())
}

// ── Image collection ────────────────────────────────────────────────────────

const MAX_IMG_LONG_EDGE: u32 = 1600;
const JPEG_QUALITY: u8 = 80;

/// Slide content-area box, in EMU (matches the "Content" placeholder's
/// `<a:xfrm>` in `slide_xml`).
const BODY_MAX_W_EMU: u32 = 8_229_600;
const BODY_MAX_H_EMU: u32 = 4_525_963;

fn collect_images(
    blocks: &[Block],
    media_index: &mut usize,
    occurrence: &mut usize,
    out: &mut Vec<PendingImage>,
) {
    for block in blocks {
        collect_images_block(block, media_index, occurrence, out);
    }
}

fn collect_images_block(
    block: &Block,
    media_index: &mut usize,
    occurrence: &mut usize,
    out: &mut Vec<PendingImage>,
) {
    match block {
        Block::Para(inlines) | Block::Heading(_, inlines) => {
            collect_images_inlines(inlines, media_index, occurrence, out);
        }
        Block::BlockQuote(blocks) => collect_images(blocks, media_index, occurrence, out),
        Block::List { items, .. } => {
            for item in items {
                collect_images(item, media_index, occurrence, out);
            }
        }
        Block::Table { head, rows } => {
            for cell in head {
                collect_images_inlines(cell, media_index, occurrence, out);
            }
            for row in rows {
                for cell in row {
                    collect_images_inlines(cell, media_index, occurrence, out);
                }
            }
        }
        _ => {}
    }
}

fn collect_images_inlines(
    inlines: &[Inline],
    media_index: &mut usize,
    occurrence: &mut usize,
    out: &mut Vec<PendingImage>,
) {
    for il in inlines {
        if let Inline::Image { src, .. } = il {
            let this_occurrence = *occurrence;
            *occurrence += 1;
            if let Some(pending) = try_load_image(src, *media_index, this_occurrence) {
                *media_index += 1;
                out.push(pending);
            }
        }
    }
}

/// Loads a local image, downsamples it if needed, and re-encodes it to a
/// compact format. Remote (http/https) and data: URIs are skipped — rudoc
/// does not fetch network resources during conversion.
fn try_load_image(src: &str, index: usize, occurrence: usize) -> Option<PendingImage> {
    if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:") {
        return None;
    }
    let path = Path::new(src);
    if !path.exists() {
        return None;
    }

    let reader = image::io::Reader::open(path).ok()?.with_guessed_format().ok()?;
    let format_is_png = matches!(reader.format(), Some(image::ImageFormat::Png));
    let img = reader.decode().ok()?;

    let (orig_w, orig_h) = (img.width(), img.height());
    let long_edge = orig_w.max(orig_h);
    let img = if long_edge > MAX_IMG_LONG_EDGE {
        let scale = MAX_IMG_LONG_EDGE as f64 / long_edge as f64;
        let new_w = ((orig_w as f64) * scale).round().max(1.0) as u32;
        let new_h = ((orig_h as f64) * scale).round().max(1.0) as u32;
        img.resize(new_w, new_h, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    let (px_w, px_h) = (img.width(), img.height());

    // PNG stays PNG (lossless, often has transparency); everything else is
    // re-encoded as JPEG at a moderate quality — the main lever for keeping
    // a deck with several embedded photos under the size budget.
    let (ext, bytes): (&'static str, Vec<u8>) = if format_is_png {
        let mut out = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new_with_quality(
            &mut out,
            image::codecs::png::CompressionType::Best,
            image::codecs::png::FilterType::Adaptive,
        );
        let rgba = img.to_rgba8();
        encoder.write_image(&rgba, px_w, px_h, image::ColorType::Rgba8).ok()?;
        ("png", out)
    } else {
        let mut out = Vec::new();
        let rgb = img.to_rgb8();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY);
        encoder.encode(&rgb, px_w, px_h, image::ColorType::Rgb8).ok()?;
        ("jpeg", out)
    };

    // EMU conversion + clamp to the slide body box, preserving aspect ratio.
    let w_emu_raw = px_w.saturating_mul(9525).max(1);
    let h_emu_raw = px_h.saturating_mul(9525).max(1);
    let scale_w = BODY_MAX_W_EMU as f64 / w_emu_raw as f64;
    let scale_h = BODY_MAX_H_EMU as f64 / h_emu_raw as f64;
    let scale = scale_w.min(scale_h).min(1.0);
    let w_emu = (w_emu_raw as f64 * scale) as u32;
    let h_emu = (h_emu_raw as f64 * scale) as u32;

    Some(PendingImage { index, occurrence, ext, bytes, w_emu, h_emu })
}

// ── XML generators ─────────────────────────────────────────────────────────

fn slide_xml(slide: &Slide, images: &[PendingImage]) -> String {
    let title_xml = xml_escape(&slide.title);
    let body_xml = blocks_to_txBody(&slide.body);

    // Images/tables render as their own top-level shapes, stacked below the
    // text placeholder, rather than living inside its <p:txBody>.
    let mut rel_id = 2usize; // rId1 is reserved for the slideLayout relationship
    let mut extra_shapes = String::new();
    let mut shape_id = 4u32; // 1=group,2=title,3=content
    let mut occurrence = 0usize;
    let mut images_iter = images.iter().peekable();
    let mut next_y: u32 = 1_600_200;
    collect_extra_shapes(
        &slide.body,
        &mut images_iter,
        &mut occurrence,
        &mut rel_id,
        &mut shape_id,
        &mut next_y,
        &mut extra_shapes,
    );

    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr>
        <a:xfrm><a:off x="0" y="0"/><a:ext cx="9144000" cy="6858000"/></a:xfrm>
      </p:grpSpPr>

      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="2" name="Title"/>
          <p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>
          <p:nvPr><p:ph type="title"/></p:nvPr>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm><a:off x="457200" y="274638"/><a:ext cx="8229600" cy="1143000"/></a:xfrm>
        </p:spPr>
        <p:txBody>
          <a:bodyPr/>
          <a:lstStyle/>
          <a:p><a:r><a:rPr lang="en-US" sz="2800" b="1"/><a:t>{title}</a:t></a:r></a:p>
        </p:txBody>
      </p:sp>

      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="3" name="Content"/>
          <p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>
          <p:nvPr><p:ph idx="1"/></p:nvPr>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm><a:off x="457200" y="1600200"/><a:ext cx="8229600" cy="4525963"/></a:xfrm>
        </p:spPr>
        <p:txBody>
          <a:bodyPr/>
          <a:lstStyle/>
          {body}
        </p:txBody>
      </p:sp>
      {extra}
    </p:spTree>
  </p:cSld>
  <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:sld>
"#,
        title = title_xml,
        body = body_xml,
        extra = extra_shapes,
    )
}

const SLIDE_BODY_BOTTOM: u32 = 1_600_200 + 4_525_963;

/// Consumes one Image-node "slot": advances `occurrence` past it and, only
/// if a successfully-loaded image exists for that *exact* occurrence number,
/// removes it from `images` and returns it. If the image at this slot failed
/// to load, `images` isn't touched, leaving it correctly aligned for the
/// next slide element that does have a match — this is what prevents a
/// later image from sliding into an earlier, failed image's position.
fn advance_and_take<'a>(
    images: &mut std::iter::Peekable<std::slice::Iter<'a, PendingImage>>,
    occurrence: &mut usize,
) -> Option<&'a PendingImage> {
    let this_occurrence = *occurrence;
    *occurrence += 1;
    if images.peek().map(|img| img.occurrence) == Some(this_occurrence) {
        images.next()
    } else {
        None
    }
}

/// Walks the same block tree as `blocks_to_txBody`, emitting a `<p:pic>` or
/// `<p:graphicFrame>` shape for each image/table, stacked vertically via
/// `next_y` so multiple such shapes on one slide don't overlap.
fn collect_extra_shapes<'a>(
    blocks: &[Block],
    images: &mut std::iter::Peekable<std::slice::Iter<'a, PendingImage>>,
    occurrence: &mut usize,
    rel_id: &mut usize,
    shape_id: &mut u32,
    next_y: &mut u32,
    out: &mut String,
) {
    for block in blocks {
        collect_extra_shapes_block(block, images, occurrence, rel_id, shape_id, next_y, out);
    }
}

fn collect_extra_shapes_block<'a>(
    block: &Block,
    images: &mut std::iter::Peekable<std::slice::Iter<'a, PendingImage>>,
    occurrence: &mut usize,
    rel_id: &mut usize,
    shape_id: &mut u32,
    next_y: &mut u32,
    out: &mut String,
) {
    match block {
        Block::Para(inlines) | Block::Heading(_, inlines) => {
            for il in inlines {
                if matches!(il, Inline::Image { .. }) {
                    if let Some(img) = advance_and_take(images, occurrence) {
                        emit_pic(img, rel_id, shape_id, *next_y, out);
                        *next_y = (*next_y + img.h_emu + 91_440).min(SLIDE_BODY_BOTTOM);
                    }
                }
            }
        }
        Block::BlockQuote(blocks) => {
            collect_extra_shapes(blocks, images, occurrence, rel_id, shape_id, next_y, out)
        }
        Block::List { items, .. } => {
            for item in items {
                collect_extra_shapes(item, images, occurrence, rel_id, shape_id, next_y, out);
            }
        }
        Block::Table { head, rows } => {
            // Table-cell images aren't rendered as separate <p:pic> shapes
            // (a table cell can't easily host one), but we must still
            // advance occurrence/images in lockstep with collect_images so
            // later images on the slide keep matching correctly.
            for cell in head {
                for il in cell {
                    if matches!(il, Inline::Image { .. }) {
                        advance_and_take(images, occurrence);
                    }
                }
            }
            for row in rows {
                for cell in row {
                    for il in cell {
                        if matches!(il, Inline::Image { .. }) {
                            advance_and_take(images, occurrence);
                        }
                    }
                }
            }
            let row_count = 1 + rows.len();
            let table_h = (row_count as u32 * 370_840).min(4_525_963);
            out.push_str(&table_to_graphic_frame(head, rows, *shape_id, *next_y, table_h));
            *shape_id += 1;
            *next_y = (*next_y + table_h + 91_440).min(SLIDE_BODY_BOTTOM);
        }
        _ => {}
    }
}

fn emit_pic(img: &PendingImage, rel_id: &mut usize, shape_id: &mut u32, y_off: u32, out: &mut String) {
    let off_x = 457200 + (BODY_MAX_W_EMU.saturating_sub(img.w_emu)) / 2;
    out.push_str(&format!(
        r#"<p:pic>
  <p:nvPicPr>
    <p:cNvPr id="{id}" name="Picture {id}"/>
    <p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr>
    <p:nvPr/>
  </p:nvPicPr>
  <p:blipFill>
    <a:blip r:embed="rId{rel}"/>
    <a:stretch><a:fillRect/></a:stretch>
  </p:blipFill>
  <p:spPr>
    <a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{w}" cy="{h}"/></a:xfrm>
    <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
  </p:spPr>
</p:pic>
"#,
        id = *shape_id,
        rel = *rel_id,
        x = off_x,
        y = y_off,
        w = img.w_emu,
        h = img.h_emu,
    ));
    *shape_id += 1;
    *rel_id += 1;
}

#[allow(non_snake_case)]
fn blocks_to_txBody(blocks: &[Block]) -> String {
    let mut out = String::new();
    if blocks.is_empty() {
        out.push_str("<a:p><a:endParaRPr/></a:p>");
        return out;
    }
    for block in blocks {
        block_to_para(block, &mut out, 0);
    }
    if out.is_empty() {
        out.push_str("<a:p><a:endParaRPr/></a:p>");
    }
    out
}

fn block_to_para(block: &Block, out: &mut String, indent: u32) {
    match block {
        Block::Para(inlines) | Block::Heading(_, inlines) => {
            // An image-only paragraph is rendered as a <p:pic> shape instead
            // (see collect_extra_shapes); skip it here to avoid an empty run.
            if inlines.len() == 1 && matches!(inlines[0], Inline::Image { .. }) {
                return;
            }
            out.push_str(&format!("<a:p><a:pPr marL=\"{}\" indent=\"0\"/>", indent * 342900));
            for il in inlines {
                inline_to_run(il, out);
            }
            out.push_str("</a:p>");
        }
        Block::CodeBlock { code, .. } => {
            for line in code.lines() {
                out.push_str("<a:p><a:pPr marL=\"342900\"/><a:r><a:rPr lang=\"en-US\" sz=\"1600\"><a:latin typeface=\"Courier New\"/></a:rPr>");
                out.push_str(&format!("<a:t>{}</a:t></a:r></a:p>", xml_escape(line)));
            }
        }
        Block::List { ordered, start, items, .. } => {
            for (i, item) in items.iter().enumerate() {
                let bullet = if *ordered {
                    format!("{}. ", i as u64 + start)
                } else {
                    "\u{2022} ".to_string()
                };
                for (bi, b) in item.iter().enumerate() {
                    if bi == 0 {
                        if let Block::Para(inlines) = b {
                            out.push_str("<a:p><a:pPr marL=\"342900\" indent=\"0\"/>");
                            out.push_str(&format!("<a:r><a:t>{}</a:t></a:r>", xml_escape(&bullet)));
                            for il in inlines { inline_to_run(il, out); }
                            out.push_str("</a:p>");
                        } else {
                            block_to_para(b, out, indent + 1);
                        }
                    } else {
                        block_to_para(b, out, indent + 1);
                    }
                }
            }
        }
        Block::HorizontalRule => {
            out.push_str("<a:p><a:r><a:t>\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}</a:t></a:r></a:p>");
        }
        Block::BlockQuote(blocks) => {
            for b in blocks { block_to_para(b, out, indent + 1); }
        }
        Block::Table { .. } => {
            // Rendered as a separate <p:graphicFrame> (see collect_extra_shapes),
            // never inside this <p:txBody> — injecting a </p:sp> here would
            // corrupt the enclosing shape's XML structure.
        }
        _ => {}
    }
}

/// Renders a table as a standalone `<p:graphicFrame>` + `<a:tbl>` (sibling
/// to the Title/Content shapes), preserving column/row structure and header
/// emphasis instead of degrading into pipe-joined text.
fn table_to_graphic_frame(
    head: &[Vec<Inline>],
    rows: &[Vec<Vec<Inline>>],
    shape_id: u32,
    y_off: u32,
    h: u32,
) -> String {
    let col_count = head.len().max(rows.iter().map(|r| r.len()).max().unwrap_or(0)).max(1);
    let total_w: u32 = 8_229_600; // matches the content placeholder width
    let col_w = total_w / col_count as u32;

    let grid_cols: String = (0..col_count)
        .map(|_| format!("<a:gridCol w=\"{}\"/>", col_w))
        .collect();

    let mut rows_xml = String::new();
    if !head.is_empty() {
        rows_xml.push_str(&table_row_xml(head, col_count, true));
    }
    for row in rows {
        rows_xml.push_str(&table_row_xml(row, col_count, false));
    }

    format!(
        r#"<p:graphicFrame>
  <p:nvGraphicFramePr>
    <p:cNvPr id="{id}" name="Table {id}"/>
    <p:cNvGraphicFramePr><a:graphicFrameLocks noGrp="1"/></p:cNvGraphicFramePr>
    <p:nvPr/>
  </p:nvGraphicFramePr>
  <p:xfrm><a:off x="457200" y="{y}"/><a:ext cx="{total_w}" cy="{h}"/></p:xfrm>
  <a:graphic>
    <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table">
      <a:tbl>
        <a:tblPr firstRow="1" bandRow="1"/>
        <a:tblGrid>{grid_cols}</a:tblGrid>
        {rows_xml}
      </a:tbl>
    </a:graphicData>
  </a:graphic>
</p:graphicFrame>
"#,
        id = shape_id, y = y_off, total_w = total_w, h = h,
        grid_cols = grid_cols, rows_xml = rows_xml,
    )
}

fn table_row_xml(cells: &[Vec<Inline>], col_count: usize, is_header: bool) -> String {
    let row_h: u32 = 370840;
    let mut tc_xml = String::new();
    for i in 0..col_count {
        let text = cells.get(i).map(|c| {
            let mut s = String::new();
            for il in c { crate::ir::doc::inline_to_text(il, &mut s); }
            s
        }).unwrap_or_default();
        let bold = if is_header { " b=\"1\"" } else { "" };
        tc_xml.push_str(&format!(
            r#"<a:tc><a:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US" sz="1400"{bold}/><a:t>{text}</a:t></a:r></a:p></a:txBody><a:tcPr/></a:tc>"#,
            bold = bold, text = xml_escape(&text),
        ));
    }
    format!("<a:tr h=\"{}\">{}</a:tr>", row_h, tc_xml)
}

fn inline_to_run(il: &Inline, out: &mut String) {
    match il {
        Inline::Text(t) => {
            if !t.is_empty() {
                out.push_str(&format!("<a:r><a:rPr lang=\"en-US\" sz=\"1800\"/><a:t>{}</a:t></a:r>", xml_escape(t)));
            }
        }
        Inline::Strong(inner) => {
            out.push_str("<a:r><a:rPr lang=\"en-US\" sz=\"1800\" b=\"1\"/><a:t>");
            let mut text = String::new();
            for il in inner { crate::ir::doc::inline_to_text(il, &mut text); }
            out.push_str(&xml_escape(&text));
            out.push_str("</a:t></a:r>");
        }
        Inline::Emph(inner) => {
            out.push_str("<a:r><a:rPr lang=\"en-US\" sz=\"1800\" i=\"1\"/><a:t>");
            let mut text = String::new();
            for il in inner { crate::ir::doc::inline_to_text(il, &mut text); }
            out.push_str(&xml_escape(&text));
            out.push_str("</a:t></a:r>");
        }
        Inline::Strikethrough(inner) => {
            out.push_str("<a:r><a:rPr lang=\"en-US\" sz=\"1800\" strike=\"sngStrike\"/><a:t>");
            let mut text = String::new();
            for il in inner { crate::ir::doc::inline_to_text(il, &mut text); }
            out.push_str(&xml_escape(&text));
            out.push_str("</a:t></a:r>");
        }
        Inline::Superscript(inner) => {
            out.push_str("<a:r><a:rPr lang=\"en-US\" sz=\"1800\" baseline=\"30000\"/><a:t>");
            let mut text = String::new();
            for il in inner { crate::ir::doc::inline_to_text(il, &mut text); }
            out.push_str(&xml_escape(&text));
            out.push_str("</a:t></a:r>");
        }
        Inline::Subscript(inner) => {
            out.push_str("<a:r><a:rPr lang=\"en-US\" sz=\"1800\" baseline=\"-25000\"/><a:t>");
            let mut text = String::new();
            for il in inner { crate::ir::doc::inline_to_text(il, &mut text); }
            out.push_str(&xml_escape(&text));
            out.push_str("</a:t></a:r>");
        }
        Inline::Code(s) => {
            out.push_str(&format!(
                "<a:r><a:rPr lang=\"en-US\" sz=\"1600\"><a:latin typeface=\"Courier New\"/></a:rPr><a:t>{}</a:t></a:r>",
                xml_escape(s)
            ));
        }
        Inline::Link { url, content, .. } => {
            // PPTX hyperlinks need relationship IDs; flatten to text for now.
            let mut text = String::new();
            for il in content { crate::ir::doc::inline_to_text(il, &mut text); }
            out.push_str(&format!("<a:r><a:t>{} ({})</a:t></a:r>", xml_escape(&text), xml_escape(url)));
        }
        Inline::Image { .. } => {
            // Rendered separately as a <p:pic> shape (see collect_extra_shapes).
        }
        Inline::LineBreak | Inline::SoftBreak => {
            out.push_str("<a:br/>");
        }
        Inline::RawInline { content, .. } if crate::ir::doc::is_html_comment(content) => {}
        other => {
            let mut text = String::new();
            crate::ir::doc::inline_to_text(other, &mut text);
            if !text.is_empty() {
                out.push_str(&format!("<a:r><a:t>{}</a:t></a:r>", xml_escape(&text)));
            }
        }
    }
}

fn presentation_xml(slide_count: usize, has_notes: bool) -> String {
    let slide_refs: String = (1..=slide_count)
        .map(|i| format!("<p:sldId id=\"{}\" r:id=\"rId{}\"/>", 255 + i, i + 5))
        .collect::<Vec<_>>()
        .join("\n      ");
    let notes_master_lst = if has_notes {
        r#"<p:notesMasterIdLst><p:notesMasterId r:id="rId3"/></p:notesMasterIdLst>"#
    } else { "" };
    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
                xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
                xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                saveSubsetFonts="1">
  <p:sldMasterIdLst>
    <p:sldMasterId id="2147483648" r:id="rId1"/>
  </p:sldMasterIdLst>
  {notes_master_lst}
  <p:sldSz cx="9144000" cy="6858000" type="screen4x3"/>
  <p:notesSz cx="6858000" cy="9144000"/>
  <p:sldIdLst>
    {slide_refs}
  </p:sldIdLst>
</p:presentation>
"#, slide_refs = slide_refs, notes_master_lst = notes_master_lst)
}

fn presentation_rels(slide_count: usize, has_notes: bool) -> String {
    let slide_rels: String = (1..=slide_count)
        .map(|i| format!(
            "<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide{}.xml\"/>",
            i + 5, i
        ))
        .collect::<Vec<_>>()
        .join("\n  ");
    let notes_master_rel = if has_notes {
        r#"<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster" Target="notesMasters/notesMaster1.xml"/>"#
    } else { "" };
    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>
  {notes_master_rel}
  {slide_rels}
</Relationships>
"#, slide_rels = slide_rels, notes_master_rel = notes_master_rel)
}

/// Per-slide relationships: layout (rId1), one per embedded image, then
/// optionally the notes slide.
fn slide_rels(slide_num: usize, images: &[PendingImage], has_notes: bool) -> String {
    let mut rels = String::new();
    rels.push_str(r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>"#);

    let mut rel_id = 2usize;
    for img in images {
        rels.push_str(&format!(
            "<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"../media/image{}.{}\"/>",
            rel_id, img.index, img.ext,
        ));
        rel_id += 1;
    }
    if has_notes {
        rels.push_str(&format!(
            "<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide\" Target=\"../notesSlides/notesSlide{}.xml\"/>",
            rel_id, slide_num,
        ));
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n{}\n</Relationships>",
        rels
    )
}

fn notes_slide_xml(notes: &str) -> String {
    let paras: String = notes
        .lines()
        .map(|line| format!("<a:p><a:r><a:rPr lang=\"en-US\"/><a:t>{}</a:t></a:r></a:p>", xml_escape(line)))
        .collect();
    let paras = if paras.is_empty() { "<a:p><a:endParaRPr/></a:p>".to_string() } else { paras };
    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:notes xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
         xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
         xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></a:xfrm></p:grpSpPr>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="2" name="Notes Placeholder"/>
          <p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>
          <p:nvPr><p:ph type="body" idx="1"/></p:nvPr>
        </p:nvSpPr>
        <p:spPr/>
        <p:txBody>
          <a:bodyPr/>
          <a:lstStyle/>
          {paras}
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
  <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:notes>
"#, paras = paras)
}

fn notes_slide_rels(slide_num: usize) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster" Target="../notesMasters/notesMaster1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="../slides/slide{}.xml"/>
</Relationships>"#,
        slide_num
    )
}

fn core_props_xml(title: &str) -> String {
    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                    xmlns:dc="http://purl.org/dc/elements/1.1/"
                    xmlns:dcterms="http://purl.org/dc/terms/"
                    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <dc:title>{title}</dc:title>
  <dc:creator>rudoc</dc:creator>
  <cp:lastModifiedBy>rudoc</cp:lastModifiedBy>
</cp:coreProperties>"#, title = xml_escape(title))
}

fn app_props_xml(slide_count: usize) -> String {
    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"
            xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">
  <Application>rudoc</Application>
  <Slides>{count}</Slides>
</Properties>"#, count = slide_count)
}

/// Escapes XML special characters and strips characters illegal in XML 1.0
/// (control chars other than tab/newline/CR) — otherwise stray control
/// bytes anywhere in the source produce non-well-formed XML that PowerPoint
/// refuses to open.
pub fn xml_escape(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .filter(|&c| matches!(c, '\t' | '\n' | '\r') || !c.is_control())
        .collect();
    cleaned
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ── Static XML templates ────────────────────────────────────────────────────

/// [Content_Types].xml must declare an Override for every slide/notesSlide
/// part actually written and a Default for every image extension actually
/// used — this used to be a `const` listing only `slide1.xml`, which made
/// any 2+ slide deck an invalid OPC package (undeclared parts).
fn content_types_xml(
    slide_count: usize,
    notes_flags: &[bool],
    any_png: bool,
    any_jpeg: bool,
) -> String {
    let slide_overrides: String = (1..=slide_count)
        .map(|i| format!(
            r#"<Override PartName="/ppt/slides/slide{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#,
            i = i
        ))
        .collect();

    let notes_overrides: String = notes_flags
        .iter()
        .enumerate()
        .filter(|(_, has)| **has)
        .map(|(idx, _)| format!(
            r#"<Override PartName="/ppt/notesSlides/notesSlide{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml"/>"#,
            i = idx + 1
        ))
        .collect();

    let notes_master_override = if notes_flags.iter().any(|b| *b) {
        r#"<Override PartName="/ppt/notesMasters/notesMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml"/>"#
    } else { "" };

    let png_default = if any_png { r#"<Default Extension="png" ContentType="image/png"/>"# } else { "" };
    let jpeg_default = if any_jpeg { r#"<Default Extension="jpeg" ContentType="image/jpeg"/>"# } else { "" };

    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  {png_default}
  {jpeg_default}
  <Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
  <Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
  {notes_master_override}
  {slide_overrides}
  {notes_overrides}
</Types>"#,
        png_default = png_default, jpeg_default = jpeg_default,
        notes_master_override = notes_master_override,
        slide_overrides = slide_overrides, notes_overrides = notes_overrides,
    )
}

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>"#;

const SLIDE_LAYOUT: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
             xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
             xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
             type="blank" preserve="1">
  <p:cSld name="Blank"><p:spTree>
    <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
    <p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></a:xfrm></p:grpSpPr>
  </p:spTree></p:cSld>
  <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:sldLayout>"#;

const SLIDE_LAYOUT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/>
</Relationships>"#;

const SLIDE_MASTER: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
             xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
             xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld><p:spTree>
    <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
    <p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></a:xfrm></p:grpSpPr>
  </p:spTree></p:cSld>
  <p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2"
            accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>
  <p:sldLayoutIdLst>
    <p:sldLayoutId id="2147483649" r:id="rId1"/>
  </p:sldLayoutIdLst>
  <p:txStyles>
    <p:titleStyle><a:lstStyle/></p:titleStyle>
    <p:bodyStyle><a:lstStyle/></p:bodyStyle>
    <p:otherStyle><a:lstStyle/></p:otherStyle>
  </p:txStyles>
</p:sldMaster>"#;

const SLIDE_MASTER_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/>
</Relationships>"#;

const THEME: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Office Theme">
  <a:themeElements>
    <a:clrScheme name="Office">
      <a:dk1><a:sysClr lastClr="000000" val="windowText"/></a:dk1>
      <a:lt1><a:sysClr lastClr="FFFFFF" val="window"/></a:lt1>
      <a:dk2><a:srgbClr val="44546A"/></a:dk2>
      <a:lt2><a:srgbClr val="E7E6E6"/></a:lt2>
      <a:accent1><a:srgbClr val="4472C4"/></a:accent1>
      <a:accent2><a:srgbClr val="ED7D31"/></a:accent2>
      <a:accent3><a:srgbClr val="A9D18E"/></a:accent3>
      <a:accent4><a:srgbClr val="FFC000"/></a:accent4>
      <a:accent5><a:srgbClr val="5A96C7"/></a:accent5>
      <a:accent6><a:srgbClr val="70AD47"/></a:accent6>
      <a:hlink><a:srgbClr val="0563C1"/></a:hlink>
      <a:folHlink><a:srgbClr val="954F72"/></a:folHlink>
    </a:clrScheme>
    <a:fontScheme name="Office">
      <a:majorFont><a:latin typeface="Calibri Light"/></a:majorFont>
      <a:minorFont><a:latin typeface="Calibri"/></a:minorFont>
    </a:fontScheme>
    <a:fmtScheme name="Office"><a:fillStyleLst>
      <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
    </a:fillStyleLst><a:lnStyleLst>
      <a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>
    </a:lnStyleLst><a:effectStyleLst>
      <a:effectStyle><a:effectLst/></a:effectStyle>
    </a:effectStyleLst><a:bgFillStyleLst>
      <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
    </a:bgFillStyleLst></a:fmtScheme>
  </a:themeElements>
</a:theme>"#;

const NOTES_MASTER: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:notesMaster xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
               xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
               xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></a:xfrm></p:grpSpPr>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="2" name="Notes Placeholder"/>
          <p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>
          <p:nvPr><p:ph type="body" idx="1"/></p:nvPr>
        </p:nvSpPr>
        <p:spPr/>
        <p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
  <p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2"
            accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>
  <p:notesStyle><a:lvl1pPr><a:defRPr sz="1200"/></a:lvl1pPr></p:notesStyle>
</p:notesMaster>"#;

const NOTES_MASTER_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/>
</Relationships>"#;
