use anyhow::{bail, Result};
use crate::ir::doc::DocIR;
use crate::writers::typst_writer;

/// Render DocIR to PDF bytes.
/// When compiled with the `pdf` feature, this drives the typst compiler in-process.
/// Without the feature, returns an error directing the user to rebuild.
pub fn render(doc: &DocIR, paper: &str, font: &str) -> Result<Vec<u8>> {
    let typ_source = typst_writer::render(doc, paper, font);
    render_from_typ(&typ_source)
}

#[cfg(feature = "pdf")]
pub fn render_from_typ(typ_source: &str) -> Result<Vec<u8>> {
    use typst_as_lib::TypstEngine;

    let engine = TypstEngine::builder()
        .main_file(typ_source)
        .build();

    let result = engine.compile()?;
    let pdf_bytes = typst_pdf::pdf(&result.output, &typst_pdf::PdfOptions::default())?;
    Ok(pdf_bytes)
}

#[cfg(not(feature = "pdf"))]
pub fn render_from_typ(_typ_source: &str) -> Result<Vec<u8>> {
    bail!(
        "PDF output requires the 'pdf' feature.\n\
         Rebuild with: cargo build --features pdf\n\
         Or output to .typ first: rudoc input.md output.typ"
    )
}
