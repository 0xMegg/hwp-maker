use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Spec {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub vars: BTreeMap<String, String>,
    pub table: Table,
}

#[derive(Debug, Deserialize)]
pub struct Table {
    #[serde(default)]
    pub border: Border,
    #[serde(default = "default_cell_padding")]
    pub cell_padding_pt: f32,
    #[serde(default)]
    pub repeat_header: bool,
    pub columns: Vec<Col>,
    pub rows: Vec<Row>,
    pub cells: Vec<Cell>,
}

fn default_cell_padding() -> f32 {
    4.0
}

#[derive(Debug, Deserialize, Clone)]
pub struct Border {
    #[serde(default = "default_border_style")]
    pub style: String,
    #[serde(default = "default_border_width")]
    pub width_pt: f32,
    #[serde(default = "default_border_color")]
    pub color: String,
}

impl Default for Border {
    fn default() -> Self {
        Border {
            style: default_border_style(),
            width_pt: default_border_width(),
            color: default_border_color(),
        }
    }
}

fn default_border_style() -> String {
    "solid".to_string()
}

fn default_border_width() -> f32 {
    0.5
}

fn default_border_color() -> String {
    "#000000".to_string()
}

#[derive(Debug, Deserialize)]
pub struct Col {
    pub width_pt: f32,
}

#[derive(Debug, Deserialize)]
pub struct Row {
    pub height_pt: f32,
}

#[derive(Debug, Deserialize)]
pub struct Cell {
    pub row: usize,
    pub col: usize,
    #[serde(default = "one")]
    pub rowspan: usize,
    #[serde(default = "one")]
    pub colspan: usize,
    pub text: Option<String>,
    pub image: Option<ImageRef>,
    #[serde(default)]
    pub style: CellStyle,
    /// If set, A방식은 이 셀의 내용을 B방식이 인식 가능한
    /// `{{name}}` (text) 혹은 `{{image:name}}` (image) placeholder 로 대체한다.
    /// vars 치환 대상이 아님.
    pub placeholder: Option<String>,
}

fn one() -> usize {
    1
}

#[derive(Debug, Deserialize)]
pub struct ImageRef {
    pub path: String,
    pub w_pt: f32,
    pub h_pt: f32,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct CellStyle {
    pub font: Option<String>,
    pub size_pt: Option<f32>,
    /// "bold" / "normal"
    pub weight: Option<String>,
    /// "left" / "center" / "right"
    pub align: Option<String>,
    /// background color, e.g. "#EEEEEE"
    pub bg: Option<String>,
    pub color: Option<String>,
}
