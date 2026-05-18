use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use rhwp::DocumentCore;

use crate::data::{DataSpec, FieldValue};
use crate::error::AppError;
use crate::html;
use crate::spec::{ImageRef, Spec};
use crate::vars;

pub fn run(spec_path: &Path, data_path: Option<&Path>, out_path: &Path) -> Result<()> {
    let text = fs::read_to_string(spec_path)
        .with_context(|| format!("read spec {}", spec_path.display()))?;
    let mut spec: Spec = serde_yaml::from_str(&text).map_err(|e| AppError::Yaml {
        file: spec_path.display().to_string(),
        source: e,
    })?;

    // vars pass (non-placeholder cells get {{var}} substitution)
    vars::expand(&mut spec);

    // data pass (optional): for cells with `placeholder: key`, merge data values
    // into the cell as real text/image so the output is the fully-rendered version
    // rather than a template carrying `{{key}}` markers.
    if let Some(dp) = data_path {
        let data_text = fs::read_to_string(dp)
            .with_context(|| format!("read data {}", dp.display()))?;
        let data: DataSpec = serde_yaml::from_str(&data_text).map_err(|e| AppError::Yaml {
            file: dp.display().to_string(),
            source: e,
        })?;
        apply_data_to_placeholders(&mut spec, &data)?;
    }

    let spec_dir = spec_path.parent().unwrap_or(Path::new("."));
    let table_html = html::build_table(&spec, spec_dir)?;

    eprintln!("generated html: {} bytes", table_html.len());
    if std::env::var("HWP_MAKER_DEBUG_HTML").is_ok() {
        eprintln!("--- HTML ---\n{}\n--- /HTML ---", table_html);
    }

    let mut core = DocumentCore::new_empty();
    core.create_blank_document_native().map_err(AppError::rhwp)?;

    let resp = core
        .paste_html_native(0, 0, 0, &table_html)
        .map_err(AppError::rhwp)?;
    eprintln!("paste_html_native -> {}", resp);

    // rhwp's `parse_table_html` emits a `raw_ctrl_data` byte layout that omits
    // the leading CommonObjAttr `attr` u32 (every field is shifted by 4 bytes),
    // so on re-parse the table ends up with `attr=0` → floating / Paper-relative.
    // Hancom Docs rejects such tables. Rebuild the bytes in proper format.
    fix_tables_for_hancom_compat(&mut core);

    // rhwp's `parse_img_html` creates Pictures via `Picture::default()`, which
    // leaves CommonObjAttr fields (treat_as_char, vert_rel_to, horz_rel_to, etc.)
    // at defaults that Hancom Docs rejects. Fix all Pictures in body and cells
    // using the recipe from `insert_picture_native` (object_ops.rs:1098).
    fix_pictures_for_hancom_compat(&mut core);

    // Patch missing BIN_DATA manifest entries.
    if std::env::var("HWP_MAKER_SKIP_BIN_PATCH").is_err() {
        register_missing_bin_data(&mut core);
    } else {
        eprintln!("(HWP_MAKER_SKIP_BIN_PATCH set — skipping BIN_DATA manifest patch)");
    }

    let bytes = core.export_hwp_native().map_err(AppError::rhwp)?;
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(out_path, &bytes).with_context(|| format!("write {}", out_path.display()))?;
    eprintln!("wrote {} ({} bytes)", out_path.display(), bytes.len());

    // roundtrip self-check
    let mut v = DocumentCore::from_bytes(&bytes).map_err(AppError::rhwp)?;
    v.convert_to_editable_native().map_err(AppError::rhwp)?;
    let paras = v
        .get_paragraph_count_native(0)
        .map_err(AppError::rhwp)?;
    eprintln!(
        "roundtrip ok: section 0 paragraphs={}, pages={}",
        paras,
        v.page_count()
    );

    Ok(())
}

/// Rebuild a Table's `raw_ctrl_data` in proper CommonObjAttr format and set
/// `common.attr` for inline ("treat_as_char") placement. Required because
/// rhwp's `parse_table_html` produces a `raw_ctrl_data` layout that omits the
/// initial `attr` u32 the parser expects, shifting every subsequent field by
/// 4 bytes and causing the table to float with defaulted positioning — which
/// Hancom Docs also apparently rejects.
fn fix_tables_for_hancom_compat(core: &mut DocumentCore) {
    use rhwp::model::control::Control;
    use rhwp::model::shape::{HorzRelTo, VertRelTo};

    let mut doc = core.document().clone();

    fn build_ctrl_data(attr: u32, width: u32, height: u32, instance_id: u32, margin: i16) -> Vec<u8> {
        let mut out = Vec::with_capacity(42);
        out.extend(&attr.to_le_bytes());
        out.extend(&0u32.to_le_bytes()); // vertical_offset
        out.extend(&0u32.to_le_bytes()); // horizontal_offset
        out.extend(&width.to_le_bytes());
        out.extend(&height.to_le_bytes());
        out.extend(&0i32.to_le_bytes()); // z_order
        out.extend(&margin.to_le_bytes()); // margin.left
        out.extend(&margin.to_le_bytes()); // margin.right
        out.extend(&margin.to_le_bytes()); // margin.top
        out.extend(&margin.to_le_bytes()); // margin.bottom
        out.extend(&instance_id.to_le_bytes());
        out.extend(&0i32.to_le_bytes()); // prevent_page_break
        out.extend(&0u16.to_le_bytes()); // description length = 0
        out
    }

    fn walk(controls: &mut Vec<rhwp::model::control::Control>, count: &mut usize) {
        for ctrl in controls.iter_mut() {
            match ctrl {
                Control::Table(tbl) => {
                    let (w, h, inst) = if tbl.raw_ctrl_data.len() >= 32 {
                        // parse_table_html layout: [8..12]=width, [12..16]=height, [28..32]=instance_id
                        let rcd = &tbl.raw_ctrl_data;
                        (
                            u32::from_le_bytes([rcd[8], rcd[9], rcd[10], rcd[11]]),
                            u32::from_le_bytes([rcd[12], rcd[13], rcd[14], rcd[15]]),
                            u32::from_le_bytes([rcd[28], rcd[29], rcd[30], rcd[31]]),
                        )
                    } else {
                        (tbl.common.width, tbl.common.height, 0x7C154B69)
                    };

                    // 0x082A2311 per rhwp's own comment: treat_as_char | vert_rel_to=Para
                    //   | horz_rel_to=Column | alignment flags.
                    let attr = 0x082A2311u32;
                    let margin = if tbl.outer_margin_left != 0 { tbl.outer_margin_left } else { 283 };

                    tbl.raw_ctrl_data = build_ctrl_data(attr, w, h, inst, margin);
                    tbl.common.attr = attr;
                    tbl.common.width = w;
                    tbl.common.height = h;
                    tbl.common.instance_id = inst;
                    tbl.common.treat_as_char = true;
                    tbl.common.vert_rel_to = VertRelTo::Para;
                    tbl.common.horz_rel_to = HorzRelTo::Column;
                    tbl.common.margin.left = margin;
                    tbl.common.margin.right = margin;
                    tbl.common.margin.top = margin;
                    tbl.common.margin.bottom = margin;

                    *count += 1;

                    // recurse into nested tables inside cells
                    for cell in &mut tbl.cells {
                        for cp in &mut cell.paragraphs {
                            walk(&mut cp.controls, count);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let mut fixed = 0usize;
    for section in &mut doc.sections {
        for para in &mut section.paragraphs {
            walk(&mut para.controls, &mut fixed);
        }
    }

    if fixed > 0 {
        eprintln!("fixed {} Table controls (CommonObjAttr recipe)", fixed);
        core.set_document(doc);
    }
}

/// Walk all Pictures and bring their CommonObjAttr / ShapeComponentAttr up to
/// Hancom-compatible values. Mirrors the recipe used by rhwp's own
/// `insert_picture_native` (object_ops.rs:1098). Also fixes the Picture's
/// containing paragraph (drop the "[이미지]" literal text, set the extended
/// control_mask bit, install a matching LineSeg) and removes the ZWSP trick
/// paragraph that was injected to convince rhwp to parse the img tag.
fn fix_pictures_for_hancom_compat(core: &mut DocumentCore) {
    use rhwp::model::control::Control;
    use rhwp::model::image::{ImageEffect, Picture};
    use rhwp::model::paragraph::{CharShapeRef, LineSeg, Paragraph};
    use rhwp::model::shape::{HorzRelTo, VertRelTo};

    type Nat = std::collections::HashMap<u16, (u32, u32)>;

    let mut doc = core.document().clone();

    let mut natural: Nat = std::collections::HashMap::new();
    for content in &doc.bin_data_content {
        if let Some((w, h)) = decode_image_dimensions(&content.data) {
            natural.insert(content.id, (w, h));
        }
    }

    fn fix_picture(pic: &mut Picture, natural: &Nat) {
        let width = pic.common.width;
        let height = pic.common.height;

        pic.common.ctrl_id = 0x67736F20; // "gso "
        pic.common.attr = 0x01 | (2 << 3) | (2 << 8) | (4 << 15) | (2 << 18);
        pic.common.treat_as_char = true;
        pic.common.vert_rel_to = VertRelTo::Para;
        pic.common.horz_rel_to = HorzRelTo::Column;
        pic.common.z_order = 0;

        pic.shape_attr.original_width = width;
        pic.shape_attr.original_height = height;
        pic.shape_attr.current_width = width;
        pic.shape_attr.current_height = height;
        pic.shape_attr.local_file_version = 1;
        pic.shape_attr.render_sx = 1.0;
        pic.shape_attr.render_sy = 1.0;

        pic.border_x = [0, 0, width as i32, 0];
        pic.border_y = [width as i32, height as i32, 0, height as i32];

        let (nat_w_px, nat_h_px) = natural
            .get(&pic.image_attr.bin_data_id)
            .copied()
            .unwrap_or(((width / 75).max(1), (height / 75).max(1)));
        pic.crop.left = 0;
        pic.crop.top = 0;
        pic.crop.right = (nat_w_px * 75) as i32;
        pic.crop.bottom = (nat_h_px * 75) as i32;

        pic.image_attr.effect = ImageEffect::RealPic;
    }

    /// Reshape a paragraph that holds a Picture so it matches `insert_picture_native`'s
    /// format: empty text, one extended-ctrl character counted (char_count=9),
    /// control_mask bit 11 set, a LineSeg sized to the picture.
    fn fix_picture_paragraph(para: &mut Paragraph) {
        let (pic_w, pic_h) = para
            .controls
            .iter()
            .find_map(|c| match c {
                Control::Picture(pic) => Some((pic.common.width, pic.common.height)),
                _ => None,
            })
            .unwrap_or((0, 0));

        let cs_id = para
            .char_shapes
            .first()
            .map(|cs| cs.char_shape_id)
            .unwrap_or(0);

        para.text = String::new();
        para.char_count = 9;
        para.control_mask = 0x00000800;
        para.char_offsets = vec![];
        if para.char_shapes.is_empty() {
            para.char_shapes.push(CharShapeRef {
                start_pos: 0,
                char_shape_id: cs_id,
            });
        }
        para.line_segs = vec![LineSeg {
            text_start: 0,
            line_height: pic_h as i32,
            text_height: pic_h as i32,
            baseline_distance: (pic_h as i32 * 850) / 1000,
            line_spacing: 600,
            segment_width: pic_w as i32,
            tag: 0x00060000,
            ..Default::default()
        }];
        para.has_para_text = true;
        para.char_count_msb = false;

        if para.raw_header_extra.len() >= 10 {
            para.raw_header_extra[0..2].copy_from_slice(&1u16.to_le_bytes());
            para.raw_header_extra[4..6].copy_from_slice(&1u16.to_le_bytes());
        } else {
            let mut rhe = vec![0u8; 10];
            rhe[0..2].copy_from_slice(&1u16.to_le_bytes());
            rhe[4..6].copy_from_slice(&1u16.to_le_bytes());
            para.raw_header_extra = rhe;
        }
    }

    fn has_picture(para: &Paragraph) -> bool {
        para.controls
            .iter()
            .any(|c| matches!(c, Control::Picture(_)))
    }

    fn is_zwsp_only(para: &Paragraph) -> bool {
        para.controls.is_empty() && para.text.contains('\u{200B}') && para.text.chars().all(|c| c == '\u{200B}')
    }

    fn fix_paragraphs(paragraphs: &mut Vec<Paragraph>, natural: &Nat, count: &mut usize) {
        let len = paragraphs.len();
        let is_pic: Vec<bool> = paragraphs.iter().map(has_picture).collect();
        let is_zwsp: Vec<bool> = paragraphs.iter().map(is_zwsp_only).collect();

        for i in 0..len {
            if is_pic[i] {
                for ctrl in paragraphs[i].controls.iter_mut() {
                    if let Control::Picture(pic) = ctrl {
                        fix_picture(pic, natural);
                        *count += 1;
                    }
                }
                fix_picture_paragraph(&mut paragraphs[i]);
            }

            // Recurse into nested tables' cell paragraphs.
            for ctrl in paragraphs[i].controls.iter_mut() {
                if let Control::Table(tbl) = ctrl {
                    for cell in &mut tbl.cells {
                        fix_paragraphs(&mut cell.paragraphs, natural, count);
                    }
                }
            }
        }

        let to_remove: Vec<usize> = (0..len)
            .filter(|&i| is_zwsp[i] && i + 1 < len && is_pic[i + 1])
            .collect();
        for &idx in to_remove.iter().rev() {
            paragraphs.remove(idx);
        }
    }

    let mut fixed = 0usize;
    for section in &mut doc.sections {
        fix_paragraphs(&mut section.paragraphs, &natural, &mut fixed);
    }

    if fixed > 0 {
        eprintln!("fixed {} Picture controls (+ paragraphs, ZWSP cleanup)", fixed);
        core.set_document(doc);
    }
}

/// Minimal PNG/JPEG header parser: returns (width_px, height_px) or None.
fn decode_image_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    // PNG: 8-byte signature then IHDR chunk at offset 8. IHDR width/height at
    // offsets 16-23 as two big-endian u32.
    if bytes.len() >= 24 && &bytes[..8] == b"\x89PNG\r\n\x1a\n" {
        let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        return Some((w, h));
    }
    // JPEG: SOI (0xFFD8) then segments. Look for a SOF marker (0xC0..=0xCF,
    // excluding 0xC4/0xC8/0xCC) and read height/width from its payload.
    if bytes.len() >= 4 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
        let mut i = 2usize;
        while i + 4 < bytes.len() {
            if bytes[i] != 0xFF {
                return None;
            }
            let marker = bytes[i + 1];
            let seg_start = i + 2;
            if matches!(marker, 0xC0..=0xCF)
                && !matches!(marker, 0xC4 | 0xC8 | 0xCC)
            {
                if seg_start + 7 >= bytes.len() {
                    return None;
                }
                let h = u16::from_be_bytes([bytes[seg_start + 3], bytes[seg_start + 4]]) as u32;
                let w = u16::from_be_bytes([bytes[seg_start + 5], bytes[seg_start + 6]]) as u32;
                return Some((w, h));
            }
            if seg_start + 2 > bytes.len() {
                return None;
            }
            let seg_len = u16::from_be_bytes([bytes[seg_start], bytes[seg_start + 1]]) as usize;
            i = seg_start + seg_len;
        }
    }
    None
}

/// Workaround for rhwp's `parse_img_html`: it adds image bytes to
/// `document.bin_data_content` but never registers a matching BIN_DATA entry in
/// DocInfo. Without a manifest entry, HWP viewers (Hancom Docs, etc.) can't
/// resolve Picture controls and the `/BinData/BINxxxx.<ext>` streams are
/// orphaned.
///
/// Approach: **surgically insert** one HWPTAG_BIN_DATA record per missing image
/// directly into `doc_info.raw_stream` (the original DocInfo bytes, preserved by
/// the parser). This keeps every other byte of DocInfo identical to what rhwp
/// loaded, avoiding rhwp's serializer rewriting DocInfo from scratch (which
/// turns out to produce output that Hancom Docs rejects).
///
/// Then we also update the rhwp `bin_data_list` model so rhwp's own cache stays
/// consistent — but we leave `raw_stream_dirty = false` so the serializer
/// returns the patched raw bytes directly.
fn register_missing_bin_data(core: &mut DocumentCore) {
    use rhwp::model::bin_data::{BinData, BinDataType};
    let mut doc = core.document().clone();
    if doc.bin_data_content.is_empty() {
        return;
    }

    // Determine which storage_ids need a manifest entry.
    let contents: Vec<(u16, String)> = doc
        .bin_data_content
        .iter()
        .filter(|c| {
            !doc.doc_info.bin_data_list.iter().any(|bd| {
                bd.data_type == BinDataType::Embedding && bd.storage_id == c.id
            })
        })
        .map(|c| (c.id, c.extension.clone()))
        .collect();
    if contents.is_empty() {
        return;
    }

    // Mirror the new entries into rhwp's model (keeps queries consistent).
    for (storage_id, extension) in &contents {
        doc.doc_info.bin_data_list.push(BinData {
            attr: 0x0001,
            data_type: BinDataType::Embedding,
            storage_id: *storage_id,
            extension: Some(extension.clone()),
            ..Default::default()
        });
    }

    // Surgical byte-level insert into raw_stream. Important: we do NOT flip
    // `raw_stream_dirty` — we want serialize_doc_info() to return these bytes
    // as-is. If raw_stream is None we skip (A방식 uses a rhwp-embedded blank
    // doc that always has raw_stream populated).
    if let Some(raw) = doc.doc_info.raw_stream.as_mut() {
        for (storage_id, extension) in &contents {
            let payload = build_bin_data_payload(*storage_id, extension);
            let record = make_record(HWPTAG_BIN_DATA, 1, &payload);
            let insert_at = find_bin_data_insertion_point(raw);
            raw.splice(insert_at..insert_at, record);
            // Update bin_data_count (first u32 in ID_MAPPINGS data).
            if let Some(idm_data_off) = find_id_mappings_data_offset(raw) {
                if idm_data_off + 4 <= raw.len() {
                    let cur = u32::from_le_bytes([
                        raw[idm_data_off],
                        raw[idm_data_off + 1],
                        raw[idm_data_off + 2],
                        raw[idm_data_off + 3],
                    ]);
                    raw[idm_data_off..idm_data_off + 4]
                        .copy_from_slice(&(cur + 1).to_le_bytes());
                }
            }
        }
        eprintln!(
            "surgically inserted {} BIN_DATA manifest entries",
            contents.len()
        );
    } else {
        eprintln!(
            "warn: doc_info.raw_stream is None; skipping BIN_DATA manifest patch \
             ({} images may not render in HWP viewers)",
            contents.len()
        );
    }

    core.set_document(doc);
}

// HWP 5.0 record format constants.
const HWPTAG_ID_MAPPINGS: u16 = 0x010 + 1;
const HWPTAG_BIN_DATA: u16 = 0x010 + 2;

/// Build record bytes: 4-byte header + optional 4-byte extended size + payload.
/// Header u32 = (size << 20) | (level << 10) | tag_id. size == 0xFFF signals
/// the real size follows in 4 extra bytes.
fn make_record(tag_id: u16, level: u16, payload: &[u8]) -> Vec<u8> {
    let size = payload.len();
    let mut out = Vec::with_capacity(4 + if size >= 0xFFF { 4 } else { 0 } + size);
    let header_size_field = if size >= 0xFFF { 0xFFFu32 } else { size as u32 };
    let header =
        (header_size_field << 20) | ((level as u32) << 10) | (tag_id as u32);
    out.extend(&header.to_le_bytes());
    if size >= 0xFFF {
        out.extend(&(size as u32).to_le_bytes());
    }
    out.extend(payload);
    out
}

/// Build HWPTAG_BIN_DATA payload for an Embedding entry.
/// Format: attr(u16) + storage_id(u16) + hwp_string(extension).
/// hwp_string = u16 length (in chars) + UTF-16LE chars.
fn build_bin_data_payload(storage_id: u16, extension: &str) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(&0x0001u16.to_le_bytes()); // attr bit 0: Embedding
    out.extend(&storage_id.to_le_bytes());
    let utf16: Vec<u16> = extension.encode_utf16().collect();
    out.extend(&(utf16.len() as u16).to_le_bytes());
    for c in &utf16 {
        out.extend(&c.to_le_bytes());
    }
    out
}

#[derive(Debug)]
struct RecPos {
    offset: usize,
    total: usize,
    tag_id: u16,
    level: u16,
    data_offset: usize,
}

fn scan_records(raw: &[u8]) -> Vec<RecPos> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos + 4 <= raw.len() {
        let header = u32::from_le_bytes([raw[pos], raw[pos + 1], raw[pos + 2], raw[pos + 3]]);
        let tag_id = (header & 0x3FF) as u16;
        let level = ((header >> 10) & 0x3FF) as u16;
        let size_field = (header >> 20) & 0xFFF;
        let (size, data_off) = if size_field == 0xFFF {
            if pos + 8 > raw.len() {
                break;
            }
            let big = u32::from_le_bytes([
                raw[pos + 4],
                raw[pos + 5],
                raw[pos + 6],
                raw[pos + 7],
            ]) as usize;
            (big, pos + 8)
        } else {
            (size_field as usize, pos + 4)
        };
        let total = (data_off - pos) + size;
        if data_off + size > raw.len() {
            break;
        }
        out.push(RecPos {
            offset: pos,
            total,
            tag_id,
            level,
            data_offset: data_off,
        });
        pos += total;
    }
    out
}

/// The insertion point for a new BIN_DATA record is: end of the last existing
/// BIN_DATA record at level 1, or immediately after ID_MAPPINGS if none.
fn find_bin_data_insertion_point(raw: &[u8]) -> usize {
    let positions = scan_records(raw);
    if let Some(last_bd) = positions.iter().rev().find(|r| r.tag_id == HWPTAG_BIN_DATA) {
        return last_bd.offset + last_bd.total;
    }
    if let Some(idm) = positions.iter().find(|r| r.tag_id == HWPTAG_ID_MAPPINGS) {
        return idm.offset + idm.total;
    }
    raw.len()
}

/// Offset of the first byte of ID_MAPPINGS data (i.e. bin_data_count u32).
fn find_id_mappings_data_offset(raw: &[u8]) -> Option<usize> {
    scan_records(raw)
        .into_iter()
        .find(|r| r.tag_id == HWPTAG_ID_MAPPINGS)
        .map(|r| r.data_offset)
}

fn apply_data_to_placeholders(spec: &mut Spec, data: &DataSpec) -> Result<(), AppError> {
    let mut matched: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();
    for cell in spec.table.cells.iter_mut() {
        let key = match &cell.placeholder {
            Some(k) => k.clone(),
            None => continue,
        };
        let value = match data.fields.get(&key) {
            Some(v) => v,
            None => continue,
        };
        match value {
            FieldValue::Text(t) => {
                cell.text = Some(t.clone());
                cell.image = None;
                cell.placeholder = None;
                matched.insert(key);
            }
            FieldValue::Image(img) => {
                // Use the data image as the cell's image. Prefer spec's size if the
                // cell already had an ImageRef (placeholder-only mode), else fall
                // back to data-provided size, else error.
                let (w, h) = match (cell.image.as_ref(), img.w_pt, img.h_pt) {
                    (Some(orig), _, _) => (orig.w_pt, orig.h_pt),
                    (None, Some(w), Some(h)) => (w, h),
                    _ => {
                        return Err(AppError::Spec(format!(
                            "image field '{}' has no size: either set w_pt/h_pt in data, \
                             or put an `image:` block on the placeholder cell in the spec",
                            key
                        )))
                    }
                };
                cell.image = Some(ImageRef {
                    path: img.image.clone(),
                    w_pt: w,
                    h_pt: h,
                });
                cell.text = None;
                cell.placeholder = None;
                matched.insert(key);
            }
        }
    }
    let unused: Vec<&String> = data
        .fields
        .keys()
        .filter(|k| !matched.contains(*k))
        .collect();
    if !unused.is_empty() {
        eprintln!(
            "warn: {} data field(s) with no matching placeholder: {:?}",
            unused.len(),
            unused
        );
    }
    Ok(())
}
