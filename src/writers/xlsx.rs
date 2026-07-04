use anyhow::Result;
use rust_xlsxwriter::{Format, Workbook};

use crate::ir::table::{CellValue, TableIR};

pub fn render(table: &TableIR) -> Result<Vec<u8>> {
    let mut workbook = Workbook::new();

    for sheet_data in &table.sheets {
        let worksheet = workbook.add_worksheet();
        worksheet.set_name(&sheet_data.name)?;

        // Bold format for headers
        let header_fmt = Format::new().set_bold();

        // Write headers
        for (col, header) in sheet_data.headers.iter().enumerate() {
            worksheet.write_string_with_format(0, col as u16, header, &header_fmt)?;
        }

        // Write data rows
        for (row_idx, row) in sheet_data.rows.iter().enumerate() {
            let excel_row = (row_idx + 1) as u32;
            for (col_idx, cell) in row.iter().enumerate() {
                let col = col_idx as u16;
                match cell {
                    CellValue::Str(s) => {
                        worksheet.write_string(excel_row, col, s)?;
                    }
                    CellValue::Num(n) => {
                        worksheet.write_number(excel_row, col, *n)?;
                    }
                    CellValue::Bool(b) => {
                        worksheet.write_boolean(excel_row, col, *b)?;
                    }
                    CellValue::Empty => {}
                }
            }
        }

        // Auto-fit columns (approximate)
        let max_col = sheet_data.headers.len().max(
            sheet_data.rows.iter().map(|r| r.len()).max().unwrap_or(0)
        );
        for col in 0..max_col {
            let width = sheet_data.headers.get(col)
                .map(|h| h.len())
                .unwrap_or(8)
                .max(8) as f64;
            worksheet.set_column_width(col as u16, width.min(30.0))?;
        }
    }

    let bytes = workbook.save_to_buffer()?;
    Ok(bytes)
}
