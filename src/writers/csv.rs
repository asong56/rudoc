use anyhow::Result;
use crate::ir::table::TableIR;

pub fn render(table: &TableIR) -> Result<String> {
    // Use the first sheet (CSV is single-sheet)
    let sheet = table.sheets.first()
        .ok_or_else(|| anyhow::anyhow!("TableIR has no sheets"))?;

    let mut wtr = csv::Writer::from_writer(Vec::new());

    // Write header row
    if !sheet.headers.is_empty() {
        wtr.write_record(&sheet.headers)?;
    }

    // Write data rows
    for row in &sheet.rows {
        let record: Vec<String> = row.iter().map(|c| c.as_str()).collect();
        wtr.write_record(&record)?;
    }

    wtr.flush()?;
    let bytes = wtr.into_inner()?;
    Ok(String::from_utf8(bytes)?)
}
