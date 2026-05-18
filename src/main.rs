use std::fs;

use anyhow::{Context, Result};
use clap::Parser;
use rhwp::DocumentCore;

use hwp_maker::cli::{Args, Cmd, DeleteCmd};
use hwp_maker::{build_cmd, delete_cmd, error::AppError, fill_cmd, read_cmd};

fn main() -> Result<()> {
    let args = Args::parse();
    match args.cmd {
        Cmd::Build { spec, data, out } => build_cmd::run(&spec, data.as_deref(), &out),
        Cmd::Fill {
            template,
            data,
            out,
        } => fill_cmd::run(&template, &data, &out),
        Cmd::Read { file } => {
            let json = read_cmd::dump_json(&file)?;
            println!("{}", json);
            Ok(())
        }
        Cmd::Inspect { file } => inspect(&file),
        Cmd::Delete(delete) => dispatch_delete(delete),
    }
}

fn dispatch_delete(cmd: DeleteCmd) -> Result<()> {
    match cmd {
        DeleteCmd::Row {
            file,
            table,
            index,
            out,
        } => delete_cmd::row(&file, table, index, &out),
        DeleteCmd::Column {
            file,
            table,
            index,
            out,
        } => delete_cmd::column(&file, table, index, &out),
        DeleteCmd::Table { file, table, out } => delete_cmd::table(&file, table, &out),
        DeleteCmd::Picture {
            file,
            table,
            cell,
            out,
        } => delete_cmd::picture_in_cell(&file, table, cell, &out),
    }
}

fn inspect(path: &std::path::Path) -> Result<()> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let mut core = DocumentCore::from_bytes(&bytes).map_err(AppError::rhwp)?;
    core.convert_to_editable_native().map_err(AppError::rhwp)?;
    println!("file: {} ({} bytes)", path.display(), bytes.len());
    println!("pages: {}", core.page_count());
    let paras = core.get_paragraph_count_native(0).map_err(AppError::rhwp)?;
    println!("section 0 paragraphs: {}", paras);
    for para_idx in 0..paras {
        if let Ok(len) = core.get_paragraph_length_native(0, para_idx) {
            println!("  para {} len={}", para_idx, len);
        }
    }
    Ok(())
}
