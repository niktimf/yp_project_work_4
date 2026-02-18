use std::path::PathBuf;

/// Application errors for image processing.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("failed to load image '{path}': {source}")]
    ImageLoad {
        path: PathBuf,
        source: image::ImageError,
    },

    #[error("failed to save image '{path}': {source}")]
    ImageSave {
        path: PathBuf,
        source: image::ImageError,
    },

    #[error("failed to load plugin '{path}': {source}")]
    PluginLoad {
        path: PathBuf,
        source: libloading::Error,
    },

    #[error(
        "failed to find symbol 'process_image' \
         in plugin: {0}"
    )]
    SymbolLoad(libloading::Error),

    #[error("plugin returned error code {code}")]
    PluginExec { code: std::ffi::c_int },

    #[error("I/O error for '{path}': {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}
