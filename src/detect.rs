use std::path::Path;
use anyhow::Result;
use crate::error::RudocError;

/// Every format Rudoc understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Html,
    Txt,
    Docx,
    Typst,
    Pdf,
    Pptx,
    Csv,
    Xlsx,
    Xml,
    Opml,
    Json,
}

impl Format {
    /// Parse a user-provided format name (case-insensitive).
    pub fn from_name(s: &str) -> Result<Self> {
        Ok(match s.to_lowercase().as_str() {
            "md" | "markdown" | "commonmark" | "gfm" => Format::Markdown,
            "html" | "htm" | "html5" => Format::Html,
            "txt" | "text" | "plain" => Format::Txt,
            "docx" | "word" => Format::Docx,
            "typ" | "typst" => Format::Typst,
            "pdf" => Format::Pdf,
            "pptx" | "powerpoint" | "ppt" => Format::Pptx,
            "csv" => Format::Csv,
            "xlsx" | "excel" | "xls" => Format::Xlsx,
            "xml" => Format::Xml,
            "opml" => Format::Opml,
            "json" => Format::Json,
            other => return Err(RudocError::UnknownFormatName(other.to_string()).into()),
        })
    }

    /// Infer format from a file path's extension.
    pub fn from_path(path: &Path) -> Result<Self> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        Self::from_name(&ext).map_err(|_| RudocError::UnknownFormatExtension(ext.clone()).into())
    }

    /// Canonical lowercase name (used in messages and as default file extension).
    pub fn name(self) -> &'static str {
        match self {
            Format::Markdown => "md",
            Format::Html => "html",
            Format::Txt => "txt",
            Format::Docx => "docx",
            Format::Typst => "typ",
            Format::Pdf => "pdf",
            Format::Pptx => "pptx",
            Format::Csv => "csv",
            Format::Xlsx => "xlsx",
            Format::Xml => "xml",
            Format::Opml => "opml",
            Format::Json => "json",
        }
    }

    /// Which IR tier does this format belong to?
    pub fn ir_tier(self) -> IrTier {
        match self {
            Format::Markdown
            | Format::Html
            | Format::Txt
            | Format::Docx
            | Format::Typst
            | Format::Pdf => IrTier::Doc,
            Format::Pptx => IrTier::Slide,
            Format::Csv | Format::Xlsx => IrTier::Table,
            Format::Xml | Format::Opml | Format::Json => IrTier::Tree,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrTier {
    Doc,
    Slide,
    Table,
    Tree,
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}
