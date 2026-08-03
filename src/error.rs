//! Structured errors with stable exit codes and machine-readable names, so
//! scripts/other tools calling rudoc can distinguish failure categories
//! without parsing human-readable message text.
use crate::detect::Format;
use thiserror::Error;

/// Process exit codes, following the BSD `sysexits.h` convention
/// (https://man.freebsd.org/cgi/man.cgi?query=sysexits) — reusing an
/// existing standard means callers that already know it (many Unix tools,
/// CI systems) need no rudoc-specific documentation to react correctly.
pub mod exit_code {
    /// Command-line usage error, or a request rudoc cannot fulfill as asked
    /// (bad/incompatible format names).
    pub const USAGE: i32 = 64;
    /// Input bytes were not valid for the declared format (e.g. non-UTF-8
    /// text input).
    pub const DATA_ERR: i32 = 65;
    /// Input file does not exist.
    pub const NO_INPUT: i32 = 66;
    /// A recognized capability isn't available in this build/environment
    /// (e.g. PDF reading, or PDF writing without `--features pdf`/typst).
    pub const UNAVAILABLE: i32 = 69;
    /// Catch-all for errors not covered by a specific variant above —
    /// anything that reaches `main` as a plain `anyhow::Error` gets this.
    pub const SOFTWARE: i32 = 70;
    /// The output file could not be created/written.
    pub const CANT_CREATE: i32 = 73;
    /// The input file exists but could not be read (permissions, etc).
    pub const IO_ERR: i32 = 74;
}

#[derive(Error, Debug)]
pub enum RudocError {
    #[error("Incompatible conversion: {from} → {to} is not supported.\nHint: both formats must belong to the same tier (doc, table, or tree), except md → pptx.")]
    IncompatibleFormats { from: Format, to: Format },

    #[error("Unsupported conversion: {from} → {to}")]
    UnsupportedConversion { from: Format, to: Format },

    #[error("Cannot read from stdin without an explicit --from format flag.")]
    UnknownStdinFormat,

    #[error("Cannot detect output format — no output file specified.\nHint: input is '{from}'. Try one of:\n  rudoc input.{from} output.<ext>\n  rudoc input.{from} -t md\n  rudoc input.{from} -t html\nRun 'rudoc --help' to see all supported formats.")]
    UnknownStdoutFormat { from: Format },

    #[error("Unknown format '{0}'. Run with --help to see supported formats.")]
    UnknownFormatName(String),

    #[error("Cannot detect format from extension '.{0}'. Use -f / -t to specify it explicitly.")]
    UnknownFormatExtension(String),

    #[error("Input is not valid UTF-8 text, which {format} conversion requires.")]
    InvalidUtf8 { format: Format },

    #[error("PDF reading (text extraction) is not implemented.\nTip: convert the PDF's source (.typ or .md) instead, if you have it.")]
    PdfReadNotImplemented,

    #[error("PDF output requires either:\n\
             • Install typst on your PATH: https://typst.app (recommended, best quality)\n\
             • Rebuild rudoc with PDF built-in: cargo build --features pdf\n\
             • Convert to .typ first then run typst manually: rudoc input.md output.typ")]
    PdfNotCompiled,

    #[error("Input file not found: {0:?}")]
    InputNotFound(std::path::PathBuf),

    #[error("Failed to read '{path:?}': {source}")]
    ReadFailed { path: std::path::PathBuf, #[source] source: std::io::Error },

    #[error("Failed to write '{path:?}': {source}")]
    WriteFailed { path: std::path::PathBuf, #[source] source: std::io::Error },
}

impl RudocError {
    /// The process exit code this error produces (see the `exit_code` module).
    pub fn exit_code(&self) -> i32 {
        use exit_code::*;
        match self {
            RudocError::IncompatibleFormats { .. }
            | RudocError::UnsupportedConversion { .. }
            | RudocError::UnknownStdinFormat
            | RudocError::UnknownStdoutFormat { .. }
            | RudocError::UnknownFormatName(_)
            | RudocError::UnknownFormatExtension(_) => USAGE,
            RudocError::InvalidUtf8 { .. } => DATA_ERR,
            RudocError::InputNotFound(_) => NO_INPUT,
            RudocError::PdfReadNotImplemented | RudocError::PdfNotCompiled => UNAVAILABLE,
            RudocError::WriteFailed { .. } => CANT_CREATE,
            RudocError::ReadFailed { .. } => IO_ERR,
        }
    }

    /// Stable, snake_case identifier for `--json` output — this string is
    /// part of rudoc's external contract and its wording will not change
    /// across versions the way the human-readable message might.
    pub fn code_name(&self) -> &'static str {
        match self {
            RudocError::IncompatibleFormats { .. } => "incompatible_formats",
            RudocError::UnsupportedConversion { .. } => "unsupported_conversion",
            RudocError::UnknownStdinFormat => "unknown_stdin_format",
            RudocError::UnknownStdoutFormat { .. } => "unknown_stdout_format",
            RudocError::UnknownFormatName(_) => "unknown_format_name",
            RudocError::UnknownFormatExtension(_) => "unknown_format_extension",
            RudocError::InvalidUtf8 { .. } => "invalid_utf8",
            RudocError::PdfReadNotImplemented => "pdf_read_not_implemented",
            RudocError::PdfNotCompiled => "pdf_not_compiled",
            RudocError::InputNotFound(_) => "input_not_found",
            RudocError::ReadFailed { .. } => "read_failed",
            RudocError::WriteFailed { .. } => "write_failed",
        }
    }
}
