use std::path::Path;

use crate::error::AppError;
use crate::html::image;
use crate::html::style;
use crate::spec::{Cell, Spec};

/// Build the full `<table>…</table>` HTML string from a parsed spec.
///
/// `spec_dir` is used to resolve image paths relative to the spec file.
pub fn build_table(spec: &Spec, spec_dir: &Path) -> Result<String, AppError> {
    validate_cells(spec)?;

    let mut out = String::new();
    out.push_str("<table style=\"border-collapse:collapse;\">\n");

    // colgroup for column widths
    out.push_str("<colgroup>");
    for col in &spec.table.columns {
        out.push_str(&format!("<col style=\"width:{}pt\">", fmt(col.width_pt)));
    }
    out.push_str("</colgroup>\n");

    let row_count = spec.table.rows.len();
    let col_count = spec.table.columns.len();

    // index cells by (row, col) -> Cell for quick lookup
    let mut grid: Vec<Vec<Option<&Cell>>> = vec![vec![None; col_count]; row_count];
    // occupied tracks cells covered by earlier rowspan/colspan cells so we skip them
    let mut occupied: Vec<Vec<bool>> = vec![vec![false; col_count]; row_count];

    for cell in &spec.table.cells {
        if cell.row >= row_count || cell.col >= col_count {
            return Err(AppError::Spec(format!(
                "cell out of bounds: row={}, col={} (rows={}, cols={})",
                cell.row, cell.col, row_count, col_count
            )));
        }
        grid[cell.row][cell.col] = Some(cell);
        for r in cell.row..(cell.row + cell.rowspan).min(row_count) {
            for c in cell.col..(cell.col + cell.colspan).min(col_count) {
                if r == cell.row && c == cell.col {
                    continue;
                }
                occupied[r][c] = true;
            }
        }
    }

    for r in 0..row_count {
        let row_height = spec.table.rows[r].height_pt;
        out.push_str(&format!("<tr style=\"height:{}pt\">", fmt(row_height)));
        for c in 0..col_count {
            if occupied[r][c] {
                continue;
            }
            let cell = match grid[r][c] {
                Some(cell) => cell,
                None => {
                    // empty placeholder cell to keep grid rectangular
                    let col_width = spec.table.columns[c].width_pt;
                    out.push_str(&format!(
                        "<td style=\"width:{}pt;height:{}pt;border:{}pt {} {};padding:{}pt\"></td>",
                        fmt(col_width),
                        fmt(row_height),
                        fmt(spec.table.border.width_pt),
                        spec.table.border.style,
                        spec.table.border.color,
                        fmt(spec.table.cell_padding_pt),
                    ));
                    continue;
                }
            };
            render_cell(&mut out, spec, r, c, cell, spec_dir)?;
        }
        out.push_str("</tr>\n");
    }

    out.push_str("</table>");
    Ok(out)
}

fn render_cell(
    out: &mut String,
    spec: &Spec,
    r: usize,
    c: usize,
    cell: &Cell,
    spec_dir: &Path,
) -> Result<(), AppError> {
    // Compute the cell's effective width/height given colspan/rowspan.
    let mut w = 0.0_f32;
    for i in 0..cell.colspan {
        let cc = c + i;
        if cc < spec.table.columns.len() {
            w += spec.table.columns[cc].width_pt;
        }
    }
    let mut h = 0.0_f32;
    for i in 0..cell.rowspan {
        let rr = r + i;
        if rr < spec.table.rows.len() {
            h += spec.table.rows[rr].height_pt;
        }
    }

    let tag = if matches!(cell.style.weight.as_deref(), Some("bold"))
        && cell.style.bg.is_some()
        && r == 0
    {
        "th"
    } else {
        "td"
    };
    let attrs = if cell.colspan > 1 || cell.rowspan > 1 {
        format!(" colspan=\"{}\" rowspan=\"{}\"", cell.colspan, cell.rowspan)
    } else {
        String::new()
    };
    let td_style = style::td_style(
        w,
        h,
        &spec.table.border,
        spec.table.cell_padding_pt,
        &cell.style,
    );
    out.push_str(&format!("<{} style=\"{}\"{}>", tag, td_style, attrs));

    // Determine cell content. Priority: `placeholder` > `image` > `text`.
    let content = if let Some(name) = &cell.placeholder {
        if cell.image.is_some() {
            format!("{{{{image:{}}}}}", name)
        } else {
            format!("{{{{{}}}}}", name)
        }
    } else if let Some(img) = &cell.image {
        let p = image::resolve(spec_dir, &img.path);
        let asset = image::load(&p)?;
        // rhwp parses width/height from HTML ATTRIBUTES (not CSS style), as pixel values
        // at DEFAULT_DPI=96: px = pt * 96/72 = pt * 4/3. px_to_hwpunit(px, 96) = px * 75.
        // So pt * 4/3 -> HWPUNIT = pt * 100 exactly.
        let w_px = img.w_pt as f64 * 4.0 / 3.0;
        let h_px = img.h_pt as f64 * 4.0 / 3.0;
        // The cell content must pass html_to_plain_text non-empty check before
        // rhwp dispatches the <img> parser. U+200B (ZERO WIDTH SPACE) is not in
        // Unicode White_Space, so str::trim() preserves it and it stays invisible.
        format!(
            "\u{200B}<img src=\"{}\" width=\"{:.3}\" height=\"{:.3}\">",
            asset.data_url, w_px, h_px,
        )
    } else {
        cell.text.clone().unwrap_or_default()
    };

    // Content kinds:
    // - pre-built HTML (contains a `<...>` tag, e.g. image emission with a ZWSP prefix)
    //   → do NOT html-escape, pass through
    // - plain text (user-provided cell.text or placeholder text like "{{name}}")
    //   → html-escape, optionally wrap in a <span> for inline styling
    let is_html = content.contains('<');
    if is_html {
        out.push_str(&content);
    } else if let Some(inner_style) = style::text_span_style(&cell.style) {
        out.push_str(&format!(
            "<span style=\"{}\">{}</span>",
            inner_style,
            html_escape(&content)
        ));
    } else {
        out.push_str(&html_escape(&content));
    }

    out.push_str(&format!("</{}>", tag));
    Ok(())
}

fn validate_cells(spec: &Spec) -> Result<(), AppError> {
    let rows = spec.table.rows.len();
    let cols = spec.table.columns.len();
    if rows == 0 || cols == 0 {
        return Err(AppError::Spec(
            "table must have at least one column and row".to_string(),
        ));
    }
    Ok(())
}

fn html_escape(s: &str) -> String {
    // `{{...}}` placeholders must not be HTML-escaped (they don't contain special chars),
    // but user text may. Treat `{` and `}` literally.
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

fn fmt(v: f32) -> String {
    let rounded = (v * 1000.0).round() / 1000.0;
    if rounded == rounded.trunc() {
        format!("{}", rounded as i64)
    } else {
        format!("{}", rounded)
    }
}
