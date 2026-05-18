use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use rhwp::DocumentCore;

use crate::data::{DataSpec, FieldValue};
use crate::error::AppError;
use crate::placeholder::{self, Ph};
use crate::traverse::{self, CellParaRef};

pub fn run(template_path: &Path, data_path: &Path, out_path: &Path) -> Result<()> {
    // 1. load template
    let tpl_bytes = fs::read(template_path)
        .with_context(|| format!("read template {}", template_path.display()))?;
    let mut core = DocumentCore::from_bytes(&tpl_bytes).map_err(AppError::rhwp)?;
    core.convert_to_editable_native().map_err(AppError::rhwp)?;
    eprintln!(
        "template loaded: {} ({} bytes), pages={}",
        template_path.display(),
        tpl_bytes.len(),
        core.page_count()
    );

    // 2. load data
    let data_text = fs::read_to_string(data_path)
        .with_context(|| format!("read data {}", data_path.display()))?;
    let data: DataSpec = serde_yaml::from_str(&data_text).map_err(|e| AppError::Yaml {
        file: data_path.display().to_string(),
        source: e,
    })?;
    eprintln!("data loaded: {} fields", data.fields.len());

    // 3. walk cells and collect placeholder hits
    let refs = traverse::walk_cell_paragraphs(&core);
    eprintln!("discovered {} cell paragraphs", refs.len());

    let mut text_replacements: Vec<(CellParaRef, String, String)> = Vec::new(); // (ref, key, new_value)
    let mut image_cell_warnings: Vec<(CellParaRef, String)> = Vec::new();
    let mut used_keys: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    for r in &refs {
        let text = match traverse::read_cell_paragraph(&core, *r) {
            Some(t) => t,
            None => continue,
        };
        match placeholder::parse(&text) {
            Some(Ph::Text(key)) => {
                let value = match data.fields.get(key).and_then(FieldValue::as_text) {
                    Some(v) => v.to_string(),
                    None => {
                        eprintln!(
                            "  warn: text placeholder {{{{{}}}}} has no text value in data",
                            key
                        );
                        continue;
                    }
                };
                used_keys.insert(key.to_string());
                text_replacements.push((*r, key.to_string(), value));
            }
            Some(Ph::Image(key)) => {
                image_cell_warnings.push((*r, key.to_string()));
            }
            None => {}
        }
    }

    // 4. perform text replacements (iterate in reverse so earlier positions stay stable
    //    — although each replacement is scoped to its own (cell, cell_para), it's still safer).
    for (r, key, new_value) in &text_replacements {
        apply_text_replacement(&mut core, *r, new_value)?;
        eprintln!(
            "  replaced text {{{{{}}}}} at sec={}, para={}, ctrl={}, cell={}, cell_para={}",
            key, r.section, r.parent_para, r.ctrl, r.cell, r.cell_para
        );
    }

    // 5. report image placeholders as unsupported in this build
    for (r, key) in &image_cell_warnings {
        eprintln!(
            "  NOTE: image placeholder {{{{image:{}}}}} at sec={}, para={}, ctrl={}, cell={}, cell_para={} \
             — skipped. rhwp public API does not currently allow inserting a Picture control into an \
             existing cell; see project notes for regeneration path.",
            key, r.section, r.parent_para, r.ctrl, r.cell, r.cell_para
        );
    }

    // 6. verify unused fields
    let unknown: Vec<&String> = data
        .fields
        .keys()
        .filter(|k| !used_keys.contains(*k))
        .collect();
    if !unknown.is_empty() {
        eprintln!(
            "  warn: {} data field(s) not matched in template: {:?}",
            unknown.len(),
            unknown
        );
    }

    // 7. export
    let bytes = core.export_hwp_native().map_err(AppError::rhwp)?;
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(out_path, &bytes).with_context(|| format!("write {}", out_path.display()))?;
    eprintln!(
        "wrote {} ({} bytes), text replacements={}, image skipped={}",
        out_path.display(),
        bytes.len(),
        text_replacements.len(),
        image_cell_warnings.len()
    );
    Ok(())
}

fn apply_text_replacement(
    core: &mut DocumentCore,
    r: CellParaRef,
    new_value: &str,
) -> Result<(), AppError> {
    // length of the current cell paragraph text (in chars)
    let len = core
        .get_cell_paragraph_length_native(r.section, r.parent_para, r.ctrl, r.cell, r.cell_para)
        .map_err(AppError::rhwp)?;

    if len > 0 {
        core.delete_text_in_cell_native(
            r.section,
            r.parent_para,
            r.ctrl,
            r.cell,
            r.cell_para,
            0,
            len,
        )
        .map_err(AppError::rhwp)?;
    }
    core.insert_text_in_cell_native(
        r.section,
        r.parent_para,
        r.ctrl,
        r.cell,
        r.cell_para,
        0,
        new_value,
    )
    .map_err(AppError::rhwp)?;
    Ok(())
}
