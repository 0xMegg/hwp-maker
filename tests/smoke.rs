//! End-to-end smoke tests: run build / fill / build-with-data through the real
//! rhwp engine and assert basic invariants on the produced HWP bytes.

use std::path::{Path, PathBuf};

use hwp_maker::{build_cmd, delete_cmd, fill_cmd, read_cmd};
use rhwp::DocumentCore;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn spec_path() -> PathBuf {
    project_root().join("examples/minimal_table.yaml")
}

fn data_path() -> PathBuf {
    project_root().join("examples/fill_data.yaml")
}

fn load_and_count(path: &Path) -> (usize, usize) {
    let bytes = std::fs::read(path).expect("read hwp");
    let mut core = DocumentCore::from_bytes(&bytes).expect("from_bytes");
    core.convert_to_editable_native().expect("convert_to_editable");
    let paras = core.get_paragraph_count_native(0).expect("paras");
    (bytes.len(), paras)
}

#[test]
fn build_template_succeeds() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let out = tmp.path().with_extension("hwp");
    build_cmd::run(&spec_path(), None, &out).expect("build");
    assert!(out.exists(), "output file not created");
    let (size, paras) = load_and_count(&out);
    assert!(size > 10_000, "template file too small: {}", size);
    assert!(paras >= 1, "no paragraphs parsed");
}

#[test]
fn build_with_data_embeds_image() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let out = tmp.path().with_extension("hwp");
    build_cmd::run(&spec_path(), Some(&data_path()), &out).expect("build --data");
    let (size, _paras) = load_and_count(&out);
    // placeholder image + template image both embedded → noticeably larger than
    // template-only build
    assert!(size > 150_000, "expected image-embedded build, got {}", size);
}

#[test]
fn fill_replaces_text_placeholder() {
    // Step 1: A방식 → template with placeholders
    let tmp_a = tempfile::NamedTempFile::new().unwrap();
    let tpl = tmp_a.path().with_extension("hwp");
    build_cmd::run(&spec_path(), None, &tpl).expect("build template");

    // Step 2: B방식 → fill
    let tmp_b = tempfile::NamedTempFile::new().unwrap();
    let out = tmp_b.path().with_extension("hwp");
    fill_cmd::run(&tpl, &data_path(), &out).expect("fill");
    assert!(out.exists());

    // Step 3: verify replacement landed
    let bytes = std::fs::read(&out).unwrap();
    let mut core = DocumentCore::from_bytes(&bytes).unwrap();
    core.convert_to_editable_native().unwrap();

    // cell=2 (row 1 col 1) held {{shoot_date}}; after fill it should contain "2026-04-23 14:30"
    let len = core
        .get_cell_paragraph_length_native(0, 0, 0, 2, 0)
        .expect("cell para length");
    let text = core
        .get_text_in_cell_native(0, 0, 0, 2, 0, 0, len)
        .expect("cell text");
    assert!(
        text.contains("2026-04-23"),
        "expected replacement text, got: {:?}",
        text
    );
    assert!(!text.contains("{{"), "placeholder still present: {:?}", text);
}

#[test]
fn build_preserves_cell_sizing_roundtrip() {
    // Build template, re-parse, and confirm the table is present with the expected
    // number of cells (6 in our spec). Exact HWPUNIT size verification requires
    // non-public APIs; this test asserts structural integrity.
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let out = tmp.path().with_extension("hwp");
    build_cmd::run(&spec_path(), None, &out).expect("build");

    let bytes = std::fs::read(&out).unwrap();
    let mut core = DocumentCore::from_bytes(&bytes).unwrap();
    core.convert_to_editable_native().unwrap();

    let refs = hwp_maker::traverse::walk_cell_paragraphs(&core);
    // 6 expected: row0 header (1) + row1 three cells (3) + row2 col0 (1) +
    // row2 col1 image cell (1, after ZWSP cleanup by fix_pictures_for_hancom_compat).
    assert_eq!(refs.len(), 6, "expected 6 cell paragraphs, got {}", refs.len());
}

#[test]
fn read_dumps_table_structure() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let out = tmp.path().with_extension("hwp");
    build_cmd::run(&spec_path(), None, &out).expect("build");

    let summary = read_cmd::dump(&out).expect("read");
    assert_eq!(summary.sections.len(), 1);
    let sec0 = &summary.sections[0];
    assert!(sec0.paragraphs.iter().any(|p| p.controls.iter().any(|c| {
        matches!(c, read_cmd::ControlSummary::Table(_))
    })), "read: no Table control found");

    // confirm all BinData entries are registered in the manifest (the bug #7 fix)
    for bd in &summary.bin_data {
        assert!(bd.manifest_registered, "BinData id={} not in manifest", bd.id);
    }
}

#[test]
fn delete_row_shrinks_table() {
    let tmp_in = tempfile::NamedTempFile::new().unwrap();
    let tpl = tmp_in.path().with_extension("hwp");
    build_cmd::run(&spec_path(), None, &tpl).expect("build");

    let tmp_out = tempfile::NamedTempFile::new().unwrap();
    let out = tmp_out.path().with_extension("hwp");
    delete_cmd::row(&tpl, 0, 1, &out).expect("delete row");

    let before = read_cmd::dump(&tpl).unwrap();
    let after = read_cmd::dump(&out).unwrap();
    let rows_before = first_table_dims(&before).0;
    let rows_after = first_table_dims(&after).0;
    assert_eq!(rows_before - 1, rows_after);
}

#[test]
fn delete_column_shrinks_table() {
    let tmp_in = tempfile::NamedTempFile::new().unwrap();
    let tpl = tmp_in.path().with_extension("hwp");
    build_cmd::run(&spec_path(), None, &tpl).expect("build");

    let tmp_out = tempfile::NamedTempFile::new().unwrap();
    let out = tmp_out.path().with_extension("hwp");
    delete_cmd::column(&tpl, 0, 2, &out).expect("delete column");

    let before = read_cmd::dump(&tpl).unwrap();
    let after = read_cmd::dump(&out).unwrap();
    let cols_before = first_table_dims(&before).1;
    let cols_after = first_table_dims(&after).1;
    assert_eq!(cols_before - 1, cols_after);
}

#[test]
fn delete_table_removes_it() {
    let tmp_in = tempfile::NamedTempFile::new().unwrap();
    let tpl = tmp_in.path().with_extension("hwp");
    build_cmd::run(&spec_path(), None, &tpl).expect("build");

    let tmp_out = tempfile::NamedTempFile::new().unwrap();
    let out = tmp_out.path().with_extension("hwp");
    delete_cmd::table(&tpl, 0, &out).expect("delete table");

    let after = read_cmd::dump(&out).unwrap();
    let table_count: usize = after
        .sections
        .iter()
        .flat_map(|s| &s.paragraphs)
        .flat_map(|p| &p.controls)
        .filter(|c| matches!(c, read_cmd::ControlSummary::Table(_)))
        .count();
    assert_eq!(table_count, 0, "table should be removed");
}

fn first_table_dims(doc: &read_cmd::DocSummary) -> (u16, u16) {
    for sec in &doc.sections {
        for para in &sec.paragraphs {
            for ctrl in &para.controls {
                if let read_cmd::ControlSummary::Table(t) = ctrl {
                    return (t.rows, t.cols);
                }
            }
        }
    }
    panic!("no table found");
}
