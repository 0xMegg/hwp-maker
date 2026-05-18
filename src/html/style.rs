use crate::spec::{Border, CellStyle};

/// Emit a CSS style string for the `<td>` of a data/header cell.
/// Sizing (`width`/`height`) is emitted in pt so rhwp's parser maps 1:1 to HWPUNIT×100.
pub fn td_style(
    width_pt: f32,
    height_pt: f32,
    border: &Border,
    padding_pt: f32,
    cs: &CellStyle,
) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(10);
    parts.push(format!("width:{}pt", fmt_pt(width_pt)));
    parts.push(format!("height:{}pt", fmt_pt(height_pt)));
    parts.push(format!(
        "border:{}pt {} {}",
        fmt_pt(border.width_pt),
        border.style,
        border.color
    ));
    parts.push(format!("padding:{}pt", fmt_pt(padding_pt)));

    if let Some(bg) = &cs.bg {
        parts.push(format!("background-color:{}", bg));
    }
    if let Some(align) = &cs.align {
        parts.push(format!("text-align:{}", align));
    }
    // vertical-align default: middle
    parts.push("vertical-align:middle".to_string());

    parts.join(";")
}

/// Emit a CSS style string for the inline wrapper around cell text content
/// (font, size, weight, color).
pub fn text_span_style(cs: &CellStyle) -> Option<String> {
    let mut parts: Vec<String> = Vec::with_capacity(4);
    if let Some(size) = cs.size_pt {
        parts.push(format!("font-size:{}pt", fmt_pt(size)));
    }
    if let Some(w) = &cs.weight {
        if w == "bold" {
            parts.push("font-weight:bold".to_string());
        }
    }
    if let Some(c) = &cs.color {
        parts.push(format!("color:{}", c));
    }
    if let Some(f) = &cs.font {
        parts.push(format!("font-family:'{}'", f));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(";"))
    }
}

fn fmt_pt(v: f32) -> String {
    // avoid trailing zeros, keep 3 decimals max
    let rounded = (v * 1000.0).round() / 1000.0;
    if rounded == rounded.trunc() {
        format!("{}", rounded as i64)
    } else {
        format!("{}", rounded)
    }
}
