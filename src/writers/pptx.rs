/// PPTX writer — generates a standards-compliant .pptx from SlideIR.
/// Produces the minimal Open XML structure required by LibreOffice / MS PowerPoint.
use anyhow::Result;
use std::io::Write;
use zip::{write::FileOptions, ZipWriter};

use crate::ir::doc::{Block, Inline};
use crate::ir::slide::{Slide, SlideIR};

pub fn render(slides: &SlideIR) -> Result<Vec<u8>> {
    let buf = Vec::new();
    let cursor = std::io::Cursor::new(buf);
    let mut zip = ZipWriter::new(cursor);
    let opts = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // [Content_Types].xml
    zip.start_file("[Content_Types].xml", opts)?;
    zip.write_all(CONTENT_TYPES.as_bytes())?;

    // _rels/.rels
    zip.start_file("_rels/.rels", opts)?;
    zip.write_all(ROOT_RELS.as_bytes())?;

    // ppt/presentation.xml
    zip.start_file("ppt/presentation.xml", opts)?;
    zip.write_all(presentation_xml(slides.slides.len()).as_bytes())?;

    // ppt/_rels/presentation.xml.rels
    zip.start_file("ppt/_rels/presentation.xml.rels", opts)?;
    zip.write_all(presentation_rels(slides.slides.len()).as_bytes())?;

    // ppt/slideLayouts/slideLayout1.xml  (shared blank layout)
    zip.start_file("ppt/slideLayouts/slideLayout1.xml", opts)?;
    zip.write_all(SLIDE_LAYOUT.as_bytes())?;

    zip.start_file("ppt/slideLayouts/_rels/slideLayout1.xml.rels", opts)?;
    zip.write_all(SLIDE_LAYOUT_RELS.as_bytes())?;

    // ppt/slideMasters/slideMaster1.xml
    zip.start_file("ppt/slideMasters/slideMaster1.xml", opts)?;
    zip.write_all(SLIDE_MASTER.as_bytes())?;

    zip.start_file("ppt/slideMasters/_rels/slideMaster1.xml.rels", opts)?;
    zip.write_all(SLIDE_MASTER_RELS.as_bytes())?;

    // ppt/theme/theme1.xml
    zip.start_file("ppt/theme/theme1.xml", opts)?;
    zip.write_all(THEME.as_bytes())?;

    // Individual slides
    for (i, slide) in slides.slides.iter().enumerate() {
        let num = i + 1;
        zip.start_file(&format!("ppt/slides/slide{}.xml", num), opts)?;
        zip.write_all(slide_xml(slide, num).as_bytes())?;

        zip.start_file(&format!("ppt/slides/_rels/slide{}.xml.rels", num), opts)?;
        zip.write_all(slide_rels().as_bytes())?;
    }

    let result = zip.finish()?;
    Ok(result.into_inner())
}

// ── XML generators ─────────────────────────────────────────────────────────

fn slide_xml(slide: &Slide, num: usize) -> String {
    let title_xml = xml_escape(&slide.title);
    let body_xml = blocks_to_txBody(&slide.body);

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

      <!-- Title -->
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

      <!-- Body -->
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
    </p:spTree>
  </p:cSld>
  <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:sld>
"#,
        title = title_xml,
        body = body_xml
    )
}

fn blocks_to_txBody(blocks: &[Block]) -> String {
    let mut out = String::new();
    if blocks.is_empty() {
        out.push_str("<a:p><a:endParaRPr/></a:p>");
        return out;
    }
    for block in blocks {
        block_to_para(block, &mut out, 0);
    }
    out
}

fn block_to_para(block: &Block, out: &mut String, indent: u32) {
    match block {
        Block::Para(inlines) | Block::Heading(_, inlines) => {
            out.push_str(&format!(
                "<a:p><a:pPr marL=\"{}\" indent=\"0\"/>",
                indent * 342900
            ));
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
                    "• ".to_string()
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
            out.push_str("<a:p><a:r><a:t>──────────────────────</a:t></a:r></a:p>");
        }
        Block::BlockQuote(blocks) => {
            for b in blocks { block_to_para(b, out, indent + 1); }
        }
        Block::Table { head, rows } => {
            // Simple flat representation in slides
            out.push_str("<a:p><a:r><a:rPr b=\"1\"/><a:t>");
            let header_text: Vec<String> = head.iter().map(|c| {
                let mut s = String::new();
                for il in c { crate::ir::doc::inline_to_text(il, &mut s); }
                s
            }).collect();
            out.push_str(&xml_escape(&header_text.join("  |  ")));
            out.push_str("</a:t></a:r></a:p>");
            for row in rows {
                let row_text: Vec<String> = row.iter().map(|c| {
                    let mut s = String::new();
                    for il in c { crate::ir::doc::inline_to_text(il, &mut s); }
                    s
                }).collect();
                out.push_str(&format!("<a:p><a:r><a:t>{}</a:t></a:r></a:p>",
                    xml_escape(&row_text.join("  |  "))));
            }
        }
        _ => {}
    }
}

fn inline_to_run(il: &Inline, out: &mut String) {
    match il {
        Inline::Text(t) => {
            if !t.is_empty() {
                out.push_str(&format!("<a:r><a:rPr lang=\"en-US\" sz=\"1800\"/><a:t>{}</a:t></a:r>",
                    xml_escape(t)));
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
        Inline::Code(s) => {
            out.push_str(&format!(
                "<a:r><a:rPr lang=\"en-US\" sz=\"1600\"><a:latin typeface=\"Courier New\"/></a:rPr><a:t>{}</a:t></a:r>",
                xml_escape(s)
            ));
        }
        Inline::Link { url, content, .. } => {
            // PPTX hyperlinks require relationship IDs; flatten to text for now
            let mut text = String::new();
            for il in content { crate::ir::doc::inline_to_text(il, &mut text); }
            out.push_str(&format!("<a:r><a:t>{} ({})</a:t></a:r>",
                xml_escape(&text), xml_escape(url)));
        }
        Inline::LineBreak | Inline::SoftBreak => {
            out.push_str("<a:br/>");
        }
        other => {
            let mut text = String::new();
            crate::ir::doc::inline_to_text(other, &mut text);
            if !text.is_empty() {
                out.push_str(&format!("<a:r><a:t>{}</a:t></a:r>", xml_escape(&text)));
            }
        }
    }
}

fn presentation_xml(slide_count: usize) -> String {
    let slide_refs: String = (1..=slide_count)
        .map(|i| format!("<p:sldId id=\"{}\" r:id=\"rId{}\"/>", 255 + i, i + 4))
        .collect::<Vec<_>>()
        .join("\n      ");
    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
                xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
                xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                saveSubsetFonts="1">
  <p:sldMasterIdLst>
    <p:sldMasterId id="2147483648" r:id="rId1"/>
  </p:sldMasterIdLst>
  <p:sldSz cx="9144000" cy="6858000" type="screen4x3"/>
  <p:notesSz cx="6858000" cy="9144000"/>
  <p:sldIdLst>
    {slide_refs}
  </p:sldIdLst>
</p:presentation>
"#, slide_refs = slide_refs)
}

fn presentation_rels(slide_count: usize) -> String {
    let slide_rels: String = (1..=slide_count)
        .map(|i| format!(
            "<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide{}.xml\"/>",
            i + 4, i
        ))
        .collect::<Vec<_>>()
        .join("\n  ");
    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>
  {slide_rels}
</Relationships>
"#, slide_rels = slide_rels)
}

fn slide_rels() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
</Relationships>"#.to_string()
}

pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ── Static XML templates ────────────────────────────────────────────────────

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
  <Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
</Types>"#;

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
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
