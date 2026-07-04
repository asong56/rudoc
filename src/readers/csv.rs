use anyhow::Result;
use crate::ir::table::{CellValue, Sheet, TableIR};

pub fn parse(src: &str, sheet_name: &str) -> Result<TableIR> {
    let mut rdr = csv::Reader::from_reader(src.as_bytes());

    let headers: Vec<String> = rdr
        .headers()?
        .iter()
        .map(|s| s.to_string())
        .collect();

    let mut rows: Vec<Vec<CellValue>> = Vec::new();
    for result in rdr.records() {
        let record = result?;
        let row: Vec<CellValue> = record
            .iter()
            .map(|field| parse_cell(field))
            .collect();
        rows.push(row);
    }

    Ok(TableIR::single(Sheet {
        name: sheet_name.to_string(),
        headers,
        rows,
    }))
}

fn parse_cell(s: &str) -> CellValue {
    if s.is_empty() {
        return CellValue::Empty;
    }
    if let Ok(n) = s.parse::<f64>() {
        return CellValue::Num(n);
    }
    match s.to_lowercase().as_str() {
        "true" | "yes" => return CellValue::Bool(true),
        "false" | "no" => return CellValue::Bool(false),
        _ => {}
    }
    CellValue::Str(s.to_string())
}
