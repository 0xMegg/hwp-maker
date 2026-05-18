use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "hwp-maker", version, about = "Create / read / update / delete HWP documents via rhwp")]
pub struct Args {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// A방식 — YAML spec → .hwp with a full table.
    /// When `--data` is provided, placeholders are replaced at generation time
    /// (embedding real images). Without `--data`, placeholders stay as
    /// `{{key}}` / `{{image:key}}` text — i.e. a template for `fill`.
    Build {
        #[arg(long)]
        spec: PathBuf,
        #[arg(long)]
        data: Option<PathBuf>,
        #[arg(long)]
        out: PathBuf,
    },

    /// B방식 — template .hwp + data YAML → .hwp with text placeholders filled.
    /// Image placeholders are skipped (rhwp public API limitation); use
    /// `build --spec ... --data ...` instead when images need to change.
    Fill {
        #[arg(long)]
        template: PathBuf,
        #[arg(long)]
        data: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },

    /// Dump the document structure as JSON (sections, paragraphs, tables,
    /// cells with text and image refs, BinData manifest state).
    Read {
        #[arg(long)]
        file: PathBuf,
    },

    /// Minimal section/paragraph summary for quick debugging.
    Inspect {
        #[arg(long)]
        file: PathBuf,
    },

    /// Structural delete operations.
    #[command(subcommand)]
    Delete(DeleteCmd),
}

#[derive(Subcommand, Debug)]
pub enum DeleteCmd {
    /// Delete a row (0-indexed) from the Nth table (default: 0).
    Row {
        #[arg(long)]
        file: PathBuf,
        #[arg(long, default_value_t = 0)]
        table: usize,
        #[arg(long)]
        index: u16,
        #[arg(long)]
        out: PathBuf,
    },
    /// Delete a column (0-indexed) from the Nth table (default: 0).
    Column {
        #[arg(long)]
        file: PathBuf,
        #[arg(long, default_value_t = 0)]
        table: usize,
        #[arg(long)]
        index: u16,
        #[arg(long)]
        out: PathBuf,
    },
    /// Delete the Nth table (and all its contents) from the document.
    Table {
        #[arg(long)]
        file: PathBuf,
        #[arg(long, default_value_t = 0)]
        table: usize,
        #[arg(long)]
        out: PathBuf,
    },
    /// Delete Picture controls from a specific cell of the Nth table.
    Picture {
        #[arg(long)]
        file: PathBuf,
        #[arg(long, default_value_t = 0)]
        table: usize,
        #[arg(long)]
        cell: usize,
        #[arg(long)]
        out: PathBuf,
    },
}
