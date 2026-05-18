//! Read / inspect a HWP file and return a structured JSON summary.
//!
//! Works directly off rhwp's `Document` model (all fields are `pub`) — no
//! mutation, no re-serialization. Safe for any HWP file that rhwp can parse.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use rhwp::{
    model::{
        control::Control,
        image::Picture,
        table::{Cell, Table},
    },
    DocumentCore,
};
use serde::Serialize;

use crate::error::AppError;

#[derive(Debug, Serialize)]
pub struct DocSummary {
    pub path: String,
    pub bytes: u64,
    pub pages: u32,
    pub sections: Vec<SectionSummary>,
    pub bin_data: Vec<BinDataSummary>,
}

#[derive(Debug, Serialize)]
pub struct SectionSummary {
    pub index: usize,
    pub paragraphs: Vec<ParagraphSummary>,
}

#[derive(Debug, Serialize)]
pub struct ParagraphSummary {
    pub index: usize,
    pub text: String,
    pub char_count: u32,
    pub controls: Vec<ControlSummary>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind")]
pub enum ControlSummary {
    Table(TableSummary),
    Picture(PictureSummary),
    Other { name: String },
}

#[derive(Debug, Serialize)]
pub struct TableSummary {
    pub control_index: usize,
    pub rows: u16,
    pub cols: u16,
    pub width_hwp: u32,
    pub height_hwp: u32,
    pub cells: Vec<CellSummary>,
}

#[derive(Debug, Serialize)]
pub struct CellSummary {
    pub index: usize,
    pub row: u16,
    pub col: u16,
    pub rowspan: u16,
    pub colspan: u16,
    pub width_hwp: u32,
    pub height_hwp: u32,
    pub paragraphs: Vec<ParagraphSummary>,
    pub field_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PictureSummary {
    pub control_index: usize,
    pub bin_data_id: u16,
    pub width_hwp: u32,
    pub height_hwp: u32,
}

#[derive(Debug, Serialize)]
pub struct BinDataSummary {
    pub id: u16,
    pub extension: String,
    pub bytes: usize,
    pub manifest_registered: bool,
}

/// Parse the file and return the structured summary.
pub fn dump(path: &Path) -> Result<DocSummary> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let mut core = DocumentCore::from_bytes(&bytes).map_err(AppError::rhwp)?;
    core.convert_to_editable_native().map_err(AppError::rhwp)?;

    let doc = core.document();

    let bin_ids_in_manifest: std::collections::HashSet<u16> = doc
        .doc_info
        .bin_data_list
        .iter()
        .filter(|bd| matches!(bd.data_type, rhwp::model::bin_data::BinDataType::Embedding))
        .map(|bd| bd.storage_id)
        .collect();

    let bin_data = doc
        .bin_data_content
        .iter()
        .map(|c| BinDataSummary {
            id: c.id,
            extension: c.extension.clone(),
            bytes: c.data.len(),
            manifest_registered: bin_ids_in_manifest.contains(&c.id),
        })
        .collect();

    let sections = doc
        .sections
        .iter()
        .enumerate()
        .map(|(i, s)| SectionSummary {
            index: i,
            paragraphs: s
                .paragraphs
                .iter()
                .enumerate()
                .map(|(pi, p)| summarize_paragraph(pi, p))
                .collect(),
        })
        .collect();

    Ok(DocSummary {
        path: path.display().to_string(),
        bytes: bytes.len() as u64,
        pages: core.page_count(),
        sections,
        bin_data,
    })
}

/// Convenience: parse the file and return pretty-printed JSON.
pub fn dump_json(path: &Path) -> Result<String> {
    let summary = dump(path)?;
    Ok(serde_json::to_string_pretty(&summary)?)
}

fn summarize_paragraph(index: usize, para: &rhwp::model::paragraph::Paragraph) -> ParagraphSummary {
    let controls = para
        .controls
        .iter()
        .enumerate()
        .map(|(ci, c)| match c {
            Control::Table(t) => ControlSummary::Table(summarize_table(ci, t)),
            Control::Picture(p) => ControlSummary::Picture(summarize_picture(ci, p)),
            other => ControlSummary::Other {
                name: control_kind_name(other).to_string(),
            },
        })
        .collect();
    ParagraphSummary {
        index,
        text: para.text.clone(),
        char_count: para.char_count,
        controls,
    }
}

fn summarize_table(control_index: usize, table: &Table) -> TableSummary {
    TableSummary {
        control_index,
        rows: table.row_count,
        cols: table.col_count,
        width_hwp: table.common.width,
        height_hwp: table.common.height,
        cells: table
            .cells
            .iter()
            .enumerate()
            .map(|(i, c)| summarize_cell(i, c))
            .collect(),
    }
}

fn summarize_cell(index: usize, cell: &Cell) -> CellSummary {
    CellSummary {
        index,
        row: cell.row,
        col: cell.col,
        rowspan: cell.row_span,
        colspan: cell.col_span,
        width_hwp: cell.width,
        height_hwp: cell.height,
        paragraphs: cell
            .paragraphs
            .iter()
            .enumerate()
            .map(|(pi, p)| summarize_paragraph(pi, p))
            .collect(),
        field_name: cell.field_name.clone(),
    }
}

fn summarize_picture(control_index: usize, pic: &Picture) -> PictureSummary {
    PictureSummary {
        control_index,
        bin_data_id: pic.image_attr.bin_data_id,
        width_hwp: pic.common.width,
        height_hwp: pic.common.height,
    }
}

fn control_kind_name(c: &Control) -> &'static str {
    match c {
        Control::Table(_) => "Table",
        Control::Picture(_) => "Picture",
        Control::Shape(_) => "Shape",
        Control::SectionDef(_) => "SectionDef",
        Control::ColumnDef(_) => "ColumnDef",
        Control::Header(_) => "Header",
        Control::Footer(_) => "Footer",
        Control::Footnote(_) => "Footnote",
        Control::Endnote(_) => "Endnote",
        Control::AutoNumber(_) => "AutoNumber",
        Control::NewNumber(_) => "NewNumber",
        Control::Field(_) => "Field",
        _ => "Other",
    }
}
