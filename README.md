# Rudoc
> A tiny, dependency-free CLI for everyday document conversions — swap formats for the files you already write, no toolchain to install.

[![Release](https://img.shields.io/github/v/release/asong56/rudoc)](https://github.com/asong56/rudoc/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)


[demo.webm](https://github.com/user-attachments/assets/c70c9286-1500-4c48-ba52-1250f0658311)

---

## What Rudoc is for

Rudoc handles the conversions people run dozens of times a day without thinking about them:

- Turn a Markdown note into a `.docx` to send someone
- Turn a `.docx` you were sent into Markdown you can diff and edit
- Drop Markdown notes into a quick `.pptx` for a stand-up
- Convert `.csv` ↔ `.xlsx`, or `.xml` ↔ `.json` ↔ `.opml`

It's a single static binary that starts instantly and does one job: move plain text, basic formatting (bold/italic/strikethrough/code/links/images/lists/tables/headings), and structured data between common formats — reliably, with no runtime to install.

**It is not a document-processing system.** There's no template engine, no math typesetting, no citation/bibliography support, and no layout engine. If your document needs any of that, Rudoc isn't the right tool — see [What Rudoc doesn't do](#what-rudoc-doesnt-do).

---

## Installation

### Download pre-built binary

| Platform | Binary |
|---|---|
| Linux x86\_64 (static musl) | `rudoc-linux-x86_64.tar.gz` |
| Windows x86\_64 | `rudoc-windows-x86_64.zip` |
| macOS Apple Silicon | `rudoc-macos-arm64.tar.gz` |

Download from [Releases](https://github.com/asong56/rudoc/releases), extract, and place on your `PATH`.

```bash
# Linux / macOS
curl -L https://github.com/asong56/rudoc/releases/latest/download/rudoc-linux-x86_64.tar.gz \
  | tar -xz -C /usr/local/bin
chmod +x /usr/local/bin/rudoc
```

### Build from source

```bash
git clone https://github.com/asong56/rudoc
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

All conversions go through a shared internal representation, so quality depends on how much a format and the IR overlap — not every pair is equally strong. Read the notes below the table before assuming a pair does what you need.

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

† PDF reading is text extraction only; layout, images, and formatting are discarded.

What "✓" means here: plain text, paragraphs, headings, bold/italic/strikethrough, inline code and code blocks, links, images, lists, tables, and blockquotes carry across. It does **not** mean full fidelity for every source document — see the format notes below.

### Presentations

```
md → pptx   (H1/H2 headings become slide boundaries)
```

A one-way, content-only export: your headings and body text become slide text boxes. There's no theme system, no layout picker, and no editing of existing `.pptx` files — Rudoc generates plain slides from Markdown, nothing more.

### Tabular data

```
csv ←→ xlsx
```

### Structured data (lossless round-trips)

```
xml ←→ opml ←→ json
```

---

## Format notes

**Markdown** — Rudoc targets a practical subset of GFM: headings, emphasis, strikethrough, code (inline/blocks), links, images, lists (including task-list checkboxes, rendered as `[x]`/`[ ]` text), tables, blockquotes, horizontal rules, and YAML frontmatter (`title`/`author`/`date`/`lang`). It is **not** a full CommonMark or Pandoc-Markdown implementation — no math (`$...$`), no citations, no custom containers, no footnotes, no definition lists.

**DOCX / PPTX** — these are content converters, not document generators. Rudoc writes text and the inline styles listed above into a plain default template (default fonts, default heading styles, no cover pages, no custom themes) and reads the same back out. It won't reproduce a specific Word template's styles, a corporate PPTX theme, or complex layouts (multi-column, text boxes, embedded charts).

**PDF** — PDF is a secondary output for quick, disposable documents, not the focus of the project. Reading a PDF back extracts text only, with no layout. See [PDF Output](#pdf-output) for how it's generated.

---

## What Rudoc doesn't do

To set expectations plainly, Rudoc does **not** aim to support:

- Math typesetting (LaTeX-style equations, MathML)
- Citations/bibliographies (BibTeX, CSL)
- Academic/scientific paper workflows
- Custom Word/PowerPoint templates or corporate themes
- Complex page layout (multi-column, running headers/footers, floats)
- Editing existing `.docx`/`.pptx` files in place
- Full fidelity round-tripping through PDF

If you need any of these, [Pandoc](https://pandoc.org) (general-purpose, LaTeX/Typst backends, citation processing) or [Typst](https://typst.app) (native typesetting) are the right tools — Rudoc deliberately doesn't try to replace them. It exists for the much smaller, much more common job of converting everyday text files without installing a toolchain.

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
  -v, --verbose             Show IR stats and timing
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

PDF is a convenience export for quick documents, not a core focus — if you need precise, print-ready layout, generate the PDF with Typst or Pandoc/LaTeX directly instead.

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

---

## Architecture

```
DocIR ────── readers/writers for: md, html, txt, docx, typ, pdf
SlideIR ──── writers for: pptx  (derived from DocIR by heading-split)
TableIR ──── readers/writers for: csv, xlsx
TreeIR ───── readers/writers for: xml, opml, json (lossless round-trips)
```

Each IR is deliberately small — enough to carry plain text and the basic formatting listed under [Format notes](#format-notes), not a general document model. That's a scope boundary, not a temporary limitation: extending an IR to also carry math, citations, or full page layout would turn Rudoc into a different (much larger) project. See [What Rudoc doesn't do](#what-rudoc-doesnt-do).

## Plan
1. *Delete .bmp support, add webp*
