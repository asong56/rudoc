use clap::{ArgAction, Parser};

#[derive(Parser, Debug)]
#[command(
    name = "rudoc",
    version = env!("CARGO_PKG_VERSION"),
    about = "Fast document converter — Pandoc's focused Rust replacement",
    long_about = "\
Convert documents between formats with zero external dependencies.\n\
Format is auto-detected from file extensions; use -f/-t to override.\n\
\n\
SUPPORTED FORMATS\n\
  Document:    md  html  txt  docx  typ  pdf\n\
  Slides:      pptx  (only from md)\n\
  Tabular:     csv  xlsx\n\
  Structured:  xml  opml  json\n\
\n\
EXAMPLES\n\
  rudoc README.md README.docx\n\
  rudoc report.docx report.pdf\n\
  rudoc notes.md slides.pptx\n\
  rudoc data.csv data.xlsx\n\
  rudoc config.json config.xml\n\
  cat page.html | rudoc -f html -t md > page.md",
    after_help = "Full docs: https://github.com/asong56/rudoc"
)]
pub struct Cli {
    /// Input file(s). Use '-' for stdin. Multiple files are merged (doc formats only).
    #[arg(value_name = "INPUT", required = false)]
    pub inputs: Vec<String>,

    /// Output file. Use '-' or omit for stdout.
    #[arg(short = 'o', long, value_name = "FILE")]
    pub output: Option<String>,

    /// Input format (overrides extension detection).
    /// Values: md  html  txt  docx  typ  pdf  pptx  csv  xlsx  xml  opml  json
    #[arg(short = 'f', long = "from", value_name = "FORMAT")]
    pub from: Option<String>,

    /// Output format (overrides extension detection).
    /// Values: md  html  txt  docx  typ  pdf  pptx  csv  xlsx  xml  opml  json
    #[arg(short = 't', long = "to", value_name = "FORMAT")]
    pub to: Option<String>,

    /// Emit a full standalone HTML document (with <head>, styles).
    #[arg(long, action = ArgAction::SetTrue)]
    pub standalone: bool,

    /// Heading level that starts a new slide for pptx output [default: 1].
    #[arg(long, value_name = "N", default_value = "1")]
    pub slide_level: u8,

    /// Sheet name for xlsx output or filter for xlsx input [default: Sheet1].
    #[arg(long, value_name = "NAME", default_value = "Sheet1")]
    pub sheet: String,

    /// Paper size for PDF/Typst output [default: a4].
    /// Common values: a4  a3  us-letter  us-legal  presentation-4-3
    #[arg(long, value_name = "SIZE", default_value = "a4")]
    pub pdf_paper: String,

    /// Body font for PDF/Typst output [default: Arial].
    #[arg(long, value_name = "NAME", default_value = "Arial")]
    pub pdf_font: String,

    /// Line-wrap width for txt output (0 = no wrapping).
    #[arg(long, value_name = "COLS")]
    pub wrap: Option<usize>,

    /// Suppress all non-error output.
    #[arg(short = 'q', long, action = ArgAction::SetTrue, conflicts_with = "verbose")]
    pub quiet: bool,

    /// Print conversion steps and timing.
    #[arg(short = 'v', long, action = ArgAction::SetTrue, conflicts_with = "quiet")]
    pub verbose: bool,
}
