use anyhow::{Context, Result};
use calamine::{open_workbook_auto_from_rs, DataType, Reader};
use std::io::Cursor;

use crate::ir::table::{CellValue, Sheet, TableIR};

pub fn parse(bytes: &[u8], target_sheet: Option<&str>) -> Result<TableIR> {
    let cursor = Cursor::new(bytes);
    let mut workbook = open_workbook_auto_from_rs(cursor)
        .context("Failed to open Excel file")?;

    let sheet_names = workbook.sheet_names().to_vec();
    let mut sheets = Vec::new();

    for name in &sheet_names {
        if let Some(filter) = target_sheet {
            if name != filter { continue; }
        }
        // worksheet_range returns Option<Result<Range, Error>>
        let range = match workbook.worksheet_range(name) {
            Some(Ok(r)) => r,
            Some(Err(e)) => {
                eprintln!("[rudoc] warning: sheet '{}' error: {}", name, e);
                continue;
            }
            None => continue,
        };

        let mut headers: Vec<String> = Vec::new();
        let mut rows: Vec<Vec<CellValue>> = Vec::new();
        let mut first_row = true;

        for row in range.rows() {
            let cells: Vec<CellValue> = row.iter().map(data_to_cell).collect();
            if first_row {
                headers = cells.iter().map(|c| c.as_str()).collect();
                first_row = false;
            } else {
                rows.push(cells);
            }
        }

        sheets.push(Sheet { name: name.clone(), headers, rows });
    }

    if sheets.is_empty() {
        anyhow::bail!("No sheets found in workbook");
    }
    Ok(TableIR { sheets })
}

fn data_to_cell(d: &DataType) -> CellValue {
    match d {
        DataType::Empty => CellValue::Empty,
        DataType::String(s) => CellValue::Str(s.clone()),
        DataType::Float(f) => CellValue::Num(*f),
        DataType::Int(i) => CellValue::Num(*i as f64),
        DataType::Bool(b) => CellValue::Bool(*b),
        DataType::Error(_) => CellValue::Empty,
        _ => CellValue::Empty,
    }
}
