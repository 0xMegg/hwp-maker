//! Structural delete operations against an existing HWP file.
//!
//! Each function loads the file, performs one delete, and writes the result.
//! All use rhwp's public delete APIs where available. Cell-internal picture
//! deletion is done by direct `Document` model manipulation because rhwp does
//! not expose a cell-scoped Picture delete.
//!
//! These operations only invalidate `section.raw_stream` — `doc_info.raw_stream`
//! stays intact — so Hancom Docs compatibility is preserved without re-running
//! the full fixup chain from `build_cmd`.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use rhwp::{model::control::Control, DocumentCore};

use crate::error::AppError;

/// Delete a row (0-indexed) from the Nth table in the document.
pub fn row(input: &Path, table_idx: usize, row_idx: u16, output: &Path) -> Result<()> {
    with_loaded(input, output, |core| {
        let (sec, para, ctrl) = locate_table(core, table_idx)?;
        core.delete_table_row_native(sec, para, ctrl, row_idx)
            .map_err(|e| AppError::rhwp(e).into())
            .map(|_| ())
    })
}

/// Delete a column (0-indexed) from the Nth table in the document.
pub fn column(input: &Path, table_idx: usize, col_idx: u16, output: &Path) -> Result<()> {
    with_loaded(input, output, |core| {
        let (sec, para, ctrl) = locate_table(core, table_idx)?;
        core.delete_table_column_native(sec, para, ctrl, col_idx)
            .map_err(|e| AppError::rhwp(e).into())
            .map(|_| ())
    })
}

/// Delete the Nth table control (and its contents) from the document.
pub fn table(input: &Path, table_idx: usize, output: &Path) -> Result<()> {
    with_loaded(input, output, |core| {
        let (sec, para, ctrl) = locate_table(core, table_idx)?;
        core.delete_table_control_native(sec, para, ctrl)
            .map_err(|e| AppError::rhwp(e).into())
            .map(|_| ())
    })
}

/// Delete Picture controls from a specific cell of the Nth table.
///
/// rhwp's public API has no cell-scoped Picture delete, so we mutate the
/// `Document` model directly: find the target cell, strip `Control::Picture`
/// entries from each of its paragraphs, and invalidate the section raw_stream.
pub fn picture_in_cell(
    input: &Path,
    table_idx: usize,
    cell_idx: usize,
    output: &Path,
) -> Result<()> {
    with_loaded(input, output, |core| {
        let (sec, para, ctrl) = locate_table(core, table_idx)?;
        let mut doc = core.document().clone();
        {
            let section = doc
                .sections
                .get_mut(sec)
                .context("section out of range")?;
            let paragraph = section
                .paragraphs
                .get_mut(para)
                .context("paragraph out of range")?;
            let table_ctrl = paragraph
                .controls
                .get_mut(ctrl)
                .context("control out of range")?;
            let table = match table_ctrl {
                Control::Table(t) => t,
                _ => anyhow::bail!("control at given position is not a table"),
            };
            let cell = table
                .cells
                .get_mut(cell_idx)
                .context("cell out of range")?;
            let before: usize = cell
                .paragraphs
                .iter()
                .map(|p| p.controls.iter().filter(|c| matches!(c, Control::Picture(_))).count())
                .sum();
            for cp in cell.paragraphs.iter_mut() {
                cp.controls.retain(|c| !matches!(c, Control::Picture(_)));
            }
            let after: usize = cell
                .paragraphs
                .iter()
                .map(|p| p.controls.iter().filter(|c| matches!(c, Control::Picture(_))).count())
                .sum();
            eprintln!(
                "  deleted {} picture(s) from cell {} of table {}",
                before - after,
                cell_idx,
                table_idx
            );
            section.raw_stream = None;
        }
        core.set_document(doc);
        Ok(())
    })
}

fn with_loaded<F>(input: &Path, output: &Path, mutate: F) -> Result<()>
where
    F: FnOnce(&mut DocumentCore) -> Result<()>,
{
    let bytes = fs::read(input).with_context(|| format!("read {}", input.display()))?;
    let mut core = DocumentCore::from_bytes(&bytes).map_err(AppError::rhwp)?;
    core.convert_to_editable_native().map_err(AppError::rhwp)?;

    mutate(&mut core)?;

    let out_bytes = core.export_hwp_native().map_err(AppError::rhwp)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(output, &out_bytes).with_context(|| format!("write {}", output.display()))?;
    eprintln!(
        "wrote {} ({} -> {} bytes)",
        output.display(),
        bytes.len(),
        out_bytes.len()
    );
    Ok(())
}

/// Resolve the Nth Table control's (section, parent_para, control_idx).
fn locate_table(core: &DocumentCore, table_idx: usize) -> Result<(usize, usize, usize)> {
    let doc = core.document();
    let mut seen = 0usize;
    for (si, section) in doc.sections.iter().enumerate() {
        for (pi, para) in section.paragraphs.iter().enumerate() {
            for (ci, ctrl) in para.controls.iter().enumerate() {
                if matches!(ctrl, Control::Table(_)) {
                    if seen == table_idx {
                        return Ok((si, pi, ci));
                    }
                    seen += 1;
                }
            }
        }
    }
    anyhow::bail!(
        "table index {} out of range — document has {} table(s)",
        table_idx,
        seen
    )
}
