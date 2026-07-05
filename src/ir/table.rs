/// Tabular data IR. Used by: csv, xlsx.
#[derive(Debug, Clone, Default)]
pub struct TableIR {
    pub sheets: Vec<Sheet>,
}

#[derive(Debug, Clone, Default)]
pub struct Sheet {
    pub name: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<CellValue>>,
}

#[derive(Debug, Clone)]
pub enum CellValue {
    Str(String),
    Num(f64),
    Bool(bool),
    Empty,
}

impl CellValue {
    pub fn as_str(&self) -> String {
        match self {
            CellValue::Str(s) => s.clone(),
            CellValue::Num(n) => {
                // Remove trailing .0 for integers
                if *n == n.floor() && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    format!("{}", n)
                }
            }
            CellValue::Bool(b) => b.to_string(),
            CellValue::Empty => String::new(),
        }
    }
}

impl TableIR {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn single(sheet: Sheet) -> Self {
        TableIR { sheets: vec![sheet] }
    }
}
