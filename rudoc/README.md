# Rudoc

**Fast, dependency-free document converter** — a focused Rust replacement for Pandoc.

[![Release](https://img.shields.io/github/v/release/your-org/rudoc)](https://github.com/your-org/rudoc/releases)
[![CI](https://github.com/your-org/rudoc/actions/workflows/ci.yml/badge.svg)](https://github.com/your-org/rudoc/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

---

## Why Rudoc?

| | Pandoc | Rudoc |
|---|---|---|
| **Runtime** | Haskell GHC runtime | None (static binary) |
| **Binary size** | ~100 MB (with GHC) | **4–6 MB** |
| **Startup** | 200–500 ms | **< 5 ms** |
| **Install** | Package manager | Drop one binary |
| **Formats** | 45+ | Focused 12 |
| **PDF engine** | LaTeX / Weasyprint / etc. | typst CLI or built-in |

Rudoc covers the 80% use-case: rich-text ↔ rich-text, spreadsheets, and structured data.

---

## Installation

### Download pre-built binary

| Platform | Binary |
|---|---|
| Linux x86\_64 (static musl) | `rudoc-linux-x86_64.tar.gz` |
| Linux ARM64 (static musl) | `rudoc-linux-aarch64.tar.gz` |
| Windows x86\_64 | `rudoc-windows-x86_64.zip` |
| macOS Apple Silicon | `rudoc-macos-aarch64.tar.gz` |
| macOS Intel | `rudoc-macos-x86_64.tar.gz` |

Download from [Releases](https://github.com/your-org/rudoc/releases), extract, and place on your `PATH`.

```bash
# Linux / macOS
curl -L https://github.com/your-org/rudoc/releases/latest/download/rudoc-linux-x86_64.tar.gz \
  | tar -xz -C /usr/local/bin
chmod +x /usr/local/bin/rudoc
```

### Build from source

```bash
git clone https://github.com/your-org/rudoc
cd rudoc

# Default (no PDF built-in, PDF works via typst CLI on PATH)
cargo build --release

# With PDF built-in (adds ~1.5 MB to binary via printpdf)
cargo build --release --features pdf

# Install to /usr/local/bin
make install
```

---

## Supported Conversions

### Document formats (fully bidirectional)

```
md  ←→  html  ←→  txt  ←→  docx  ←→  typ  ←→  pdf
```

| | md | html | txt | docx | typ | pdf |
|---|---|---|---|---|---|---|
| **md** | — | ✓ | ✓ | ✓ | ✓ | ✓ |
| **html** | ✓ | — | ✓ | ✓ | ✓ | ✓ |
| **txt** | ✓ | ✓ | — | ✓ | ✓ | ✓ |
| **docx** | ✓ | ✓ | ✓ | — | ✓ | ✓ |
| **typ** | ✓ | ✓ | ✓ | ✓ | — | ✓ |
| **pdf** *(read)* | ✓† | ✓† | ✓† | — | — | — |

† PDF reading is text extraction only; layout is not preserved.

### Presentations

```
md → pptx   (H1/H2 headings become slide boundaries)
```

### Tabular data

```
csv ←→ xlsx
```

### Structured data (lossless round-trips)

```
xml ←→ opml ←→ json
```

---

## Usage

```bash
# Auto-detect formats from file extensions — no flags needed
rudoc README.md README.docx
rudoc report.docx report.pdf
rudoc notes.md slides.pptx
rudoc data.csv report.xlsx
rudoc config.json schema.xml

# Explicit format override
rudoc -f md -t html README.md
rudoc --from docx --to markdown report.docx

# Output to explicit path
rudoc README.md -o /tmp/readme.pdf

# Pipe-friendly (stdin/stdout)
cat README.md | rudoc -f md -t html > page.html
rudoc -f json -t xml < data.json > data.xml

# Merge multiple Markdown files into one document
rudoc ch1.md ch2.md ch3.md -t html
rudoc ch1.md ch2.md -t docx -o book.docx

# Slide level control (default: H1 = new slide)
rudoc notes.md slides.pptx --slide-level 2

# Verbose output
rudoc README.md -t html -v
```

### Options

```
OPTIONS:
  -f, --from <FORMAT>      Input format (auto-detected from extension)
  -t, --to <FORMAT>        Output format (auto-detected from extension)
  -o, --output <FILE>      Output path (default: stdout)
      --standalone         Emit full HTML document with <head> and CSS
      --slide-level <N>    Heading level that starts a new slide [default: 1]
      --sheet <NAME>       Sheet name for XLSX output/input [default: Sheet1]
      --pdf-paper <SIZE>   Paper: a4 a3 a5 us-letter us-legal [default: a4]
      --pdf-font <NAME>    Body font for Typst/PDF output [default: Arial]
      --wrap <COLS>        Line-wrap width for md/txt output (0 = off)
  -q, --quiet              Suppress progress messages
  -v, --verbose            Show IR stats and timing
  -h, --help               Print help
      --version            Print version

FORMAT NAMES (case-insensitive):
  md / markdown / commonmark / gfm
  html / htm / html5
  txt / text / plain
  docx / word
  typ / typst
  pdf
  pptx / powerpoint / ppt
  csv
  xlsx / excel / xls
  xml
  opml
  json
```

---

## PDF Output

Rudoc tries three strategies in order:

1. **typst CLI** (best quality) — if `typst` is on your `PATH`
   ```bash
   # Install typst: https://typst.app
   # macOS:  brew install typst
   # Linux:  snap install typst
   # Windows: winget install typst.typst
   ```
2. **Built-in printpdf** (always available, no external tools) — compile with `--features pdf`
3. **Helpful error** with instructions

To get the highest-quality PDFs:
```bash
# Install typst, then:
rudoc document.md -o document.pdf
```

---

## Building Cross-Platform Binaries

### Using `cross` (recommended, needs Docker)

```bash
cargo install cross

# Linux x86_64 musl (static)
cross build --release --features pdf --target x86_64-unknown-linux-musl

# Linux ARM64 musl (static)  
cross build --release --features pdf --target aarch64-unknown-linux-musl

# Windows x86_64
cross build --release --features pdf --target x86_64-pc-windows-gnu
```

### macOS (native build required)

```bash
# Apple Silicon
cargo build --release --features pdf --target aarch64-apple-darwin

# Intel (or universal binary)
cargo build --release --features pdf --target x86_64-apple-darwin

# Universal fat binary
lipo -create \
  target/aarch64-apple-darwin/release/rudoc \
  target/x86_64-apple-darwin/release/rudoc \
  -output rudoc-macos-universal
```

### GitHub Actions

Push a version tag to trigger the full release pipeline:

```bash
git tag v0.1.0
git push origin v0.1.0
```

This builds and releases binaries for all 6 platform/arch combinations automatically.

---

## Architecture

```
DocIR ────── readers/writers for: md, html, txt, docx, typ, pdf
SlideIR ──── writers for: pptx  (derived from DocIR by heading-split)
TableIR ──── readers/writers for: csv, xlsx
TreeIR ───── readers/writers for: xml, opml, json (lossless round-trips)
```

---

## License

MIT — see [LICENSE](LICENSE)
