use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("rhwp error: {0}")]
    Rhwp(String),

    #[error("yaml error at {file}: {source}")]
    Yaml {
        file: String,
        #[source]
        source: serde_yaml::Error,
    },

    #[error("io error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("image error at {path}: {source}")]
    Image {
        path: String,
        #[source]
        source: image::ImageError,
    },

    #[error("invalid spec: {0}")]
    Spec(String),

    #[error("unknown field '{0}' in data yaml")]
    UnknownField(String),

    #[error("template has {count} placeholders unsatisfied: {sample}")]
    UnresolvedPlaceholders { count: usize, sample: String },
}

impl AppError {
    pub fn rhwp(e: rhwp::HwpError) -> Self {
        AppError::Rhwp(e.to_string())
    }
}
