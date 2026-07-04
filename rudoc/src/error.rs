use thiserror::Error;
use crate::detect::Format;

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum RudocError {
    #[error("Incompatible conversion: {from} → {to} is not supported.\nHint: both formats must belong to the same tier (doc, table, or tree), except md → pptx.")]
    IncompatibleFormats { from: Format, to: Format },

    #[error("Cannot read from stdin without explicit --from format flag.")]
    UnknownStdinFormat,

    #[error("Cannot write to stdout without explicit --to format flag.")]
    UnknownStdoutFormat,

    #[error("PDF reading is limited to text extraction. Some layout information will be lost.")]
    PdfReadLossyWarning,

    #[error("The --pdf feature is not compiled in. Rebuild with: cargo build --features pdf")]
    PdfNotCompiled,

    #[error("Unsupported conversion: {from} → {to}")]
    UnsupportedConversion { from: Format, to: Format },
}
