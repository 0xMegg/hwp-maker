//! hwp-maker — HWP/HWPX document CRUD via the `rhwp` crate.
//!
//! Library consumers typically want the four high-level entry points re-exported
//! below. For advanced use (custom CLI, bypassing spec YAML, etc.) the inner
//! modules are also public.
//!
//! ```no_run
//! use std::path::Path;
//! use hwp_maker::{build, fill, read_as_json, delete};
//!
//! # fn main() -> anyhow::Result<()> {
//! // A방식: spec YAML → .hwp
//! build(Path::new("spec.yaml"), None, Path::new("out.hwp"))?;
//!
//! // A방식: spec YAML + data YAML → fully-rendered .hwp
//! build(Path::new("spec.yaml"), Some(Path::new("data.yaml")), Path::new("out.hwp"))?;
//!
//! // B방식: template + data → text placeholders replaced
//! fill(Path::new("template.hwp"), Path::new("data.yaml"), Path::new("out.hwp"))?;
//!
//! // Read structure
//! let json = read_as_json(Path::new("out.hwp"))?;
//! println!("{}", json);
//!
//! // Delete operations
//! delete::row(Path::new("in.hwp"), 0, 1, Path::new("out.hwp"))?;    // table 0, row 1
//! delete::column(Path::new("in.hwp"), 0, 2, Path::new("out.hwp"))?; // table 0, col 2
//! # Ok(()) }
//! ```

pub mod build_cmd;
pub mod cli;
pub mod data;
pub mod delete_cmd;
pub mod error;
pub mod fill_cmd;
pub mod html;
pub mod placeholder;
pub mod read_cmd;
pub mod spec;
pub mod traverse;
pub mod vars;

// -------------------------------------------------------------------------
// Library-facing re-exports
// -------------------------------------------------------------------------

pub use build_cmd::run as build;
pub use fill_cmd::run as fill;
pub use read_cmd::dump_json as read_as_json;

/// Structural delete operations against an existing HWP file.
pub mod delete {
    pub use crate::delete_cmd::{column, picture_in_cell, row, table as table_control};
}

// rhwp re-export for consumers who want direct access to the engine.
pub use rhwp;
