use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct DataSpec {
    #[serde(default)]
    pub version: u32,
    pub fields: BTreeMap<String, FieldValue>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum FieldValue {
    Text(String),
    Image(ImageValue),
}

#[derive(Debug, Deserialize)]
pub struct ImageValue {
    pub image: String,
    pub w_pt: Option<f32>,
    pub h_pt: Option<f32>,
}

impl FieldValue {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            FieldValue::Text(s) => Some(s.as_str()),
            FieldValue::Image(_) => None,
        }
    }

    pub fn as_image(&self) -> Option<&ImageValue> {
        match self {
            FieldValue::Image(i) => Some(i),
            FieldValue::Text(_) => None,
        }
    }
}
