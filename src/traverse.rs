use rhwp::DocumentCore;

/// Handle to a cell paragraph, addressable via the public rhwp API.
#[derive(Debug, Clone, Copy)]
pub struct CellParaRef {
    pub section: usize,
    pub parent_para: usize,
    pub ctrl: usize,
    pub cell: usize,
    pub cell_para: usize,
}

/// Walk all cell paragraphs reachable from the public rhwp API by probing.
///
/// Strategy: iterate section 0..N (stop on error), each paragraph 0..M,
/// each control 0..C (probe via `get_cell_paragraph_count_native`),
/// each cell 0..K (probe same), each cell paragraph 0..P.
///
/// For v1 templates (A-방식 generated), the traversal is effectively
/// `sec=0, para=0, ctrl=0`, but the probe is robust to other layouts.
pub fn walk_cell_paragraphs(core: &DocumentCore) -> Vec<CellParaRef> {
    let mut out: Vec<CellParaRef> = Vec::new();
    let mut sec = 0usize;
    loop {
        let para_count = match core.get_paragraph_count_native(sec) {
            Ok(n) => n,
            Err(_) => break,
        };
        for parent_para in 0..para_count {
            // Probe controls. There's no public control-count API, so we
            // probe `get_cell_paragraph_count_native(sec, para, ctrl, 0)`.
            // An Err means either "not a table" or "ctrl out of range" —
            // both end the probe for this paragraph.
            let mut ctrl = 0usize;
            loop {
                match core.get_cell_paragraph_count_native(sec, parent_para, ctrl, 0) {
                    Ok(_first_cell_paras) => {
                        // walk cells
                        let mut cell = 0usize;
                        loop {
                            match core.get_cell_paragraph_count_native(sec, parent_para, ctrl, cell)
                            {
                                Ok(cell_para_count) => {
                                    for cell_para in 0..cell_para_count {
                                        out.push(CellParaRef {
                                            section: sec,
                                            parent_para,
                                            ctrl,
                                            cell,
                                            cell_para,
                                        });
                                    }
                                    cell += 1;
                                }
                                Err(_) => break,
                            }
                        }
                        ctrl += 1;
                    }
                    Err(_) => break,
                }
            }
        }
        sec += 1;
    }
    out
}

/// Read the full text of a cell paragraph.
pub fn read_cell_paragraph(core: &DocumentCore, r: CellParaRef) -> Option<String> {
    let len = core
        .get_cell_paragraph_length_native(r.section, r.parent_para, r.ctrl, r.cell, r.cell_para)
        .ok()?;
    core.get_text_in_cell_native(
        r.section,
        r.parent_para,
        r.ctrl,
        r.cell,
        r.cell_para,
        0,
        len,
    )
    .ok()
}
