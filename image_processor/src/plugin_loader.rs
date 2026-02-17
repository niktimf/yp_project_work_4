use std::ffi::CString;
use std::path::{Path, PathBuf};

use libloading::{Library, Symbol};

use crate::error::AppError;

/// Plugin function type matching the C signature:
/// `void process_image(uint32_t width, uint32_t height,
///                     uint8_t* rgba_data, const char* params)`
type ProcessImageFn =
    unsafe extern "C" fn(u32, u32, *mut u8, *const std::ffi::c_char);

/// Plugin loader — wraps a dynamic library and provides
/// a safe interface for calling `process_image`.
pub struct PluginLoader {
    _library: Library,
    process_fn: ProcessImageFn,
}

impl PluginLoader {
    /// Loads a plugin by name from the specified directory.
    ///
    /// Constructs a platform-specific library filename:
    /// - Linux: `lib{name}.so`
    /// - Windows: `{name}.dll`
    /// - macOS: `lib{name}.dylib`
    ///
    /// # Errors
    ///
    /// Returns `AppError::PluginLoad` if the library file
    /// cannot be loaded, or `AppError::SymbolLoad` if the
    /// `process_image` symbol is not found.
    pub fn load(
        plugin_name: &str,
        plugin_dir: &Path,
    ) -> Result<Self, AppError> {
        let lib_path = library_path(plugin_name, plugin_dir);

        log::info!("Loading plugin: {}", lib_path.display());

        // SAFETY: loading a dynamic library is inherently unsafe as we trust external code.
        // The library must be compiled from trusted source code.
        let library = unsafe { Library::new(&lib_path) }.map_err(|source| {
            AppError::PluginLoad {
                path: lib_path.clone(),
                source,
            }
        })?;

        // SAFETY: we load a symbol with a known C signature.
        // Signature correctness is guaranteed by the plugin API convention.
        let process_fn = unsafe {
            let sym: Symbol<'_, ProcessImageFn> = library
                .get(b"process_image")
                .map_err(AppError::SymbolLoad)?;
            *sym
        };

        Ok(Self {
            _library: library,
            process_fn,
        })
    }

    /// Calls the plugin function to process an image.
    ///
    /// # Arguments
    /// - `width`, `height` — image dimensions in pixels
    /// - `rgba_data` — mutable RGBA pixel buffer
    ///   (length = width * height * 4)
    /// - `params` — parameter string for the plugin
    pub fn process_image(
        &self,
        width: u32,
        height: u32,
        rgba_data: &mut [u8],
        params: &str,
    ) {
        let params_cstring = CString::new(params).unwrap_or_default();

        log::debug!(
            "Calling plugin: {}x{}, {} bytes, params={:?}",
            width,
            height,
            rgba_data.len(),
            params
        );

        // SAFETY: we pass a valid pointer to image data and a C string for parameters.
        // The rgba_data buffer remains alive for the entire call.
        // Buffer size = width * height * 4 bytes.
        unsafe {
            (self.process_fn)(
                width,
                height,
                rgba_data.as_mut_ptr(),
                params_cstring.as_ptr(),
            );
        }
    }
}

/// Target operating system for library name resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Os {
    Linux,
    Windows,
    MacOs,
}

impl Os {
    /// Returns the OS corresponding to the current
    /// compilation target.
    const fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Linux
        }
    }
}

/// Returns the platform-specific library filename.
fn library_filename(name: &str, os: Os) -> String {
    match os {
        Os::Windows => format!("{name}.dll"),
        Os::MacOs => format!("lib{name}.dylib"),
        Os::Linux => format!("lib{name}.so"),
    }
}

/// Constructs the full path to a plugin library file
/// based on the current OS.
fn library_path(name: &str, dir: &Path) -> PathBuf {
    dir.join(library_filename(name, Os::current()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(Os::Linux, "invert", "libinvert.so")]
    #[case(Os::Linux, "blur", "libblur.so")]
    #[case(Os::Windows, "invert", "invert.dll")]
    #[case(Os::Windows, "mirror", "mirror.dll")]
    #[case(Os::MacOs, "invert", "libinvert.dylib")]
    #[case(Os::MacOs, "blur", "libblur.dylib")]
    fn library_filename_for_os(
        #[case] os: Os,
        #[case] name: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(library_filename(name, os), expected);
    }

    #[test]
    fn library_path_joins_dir_and_filename() {
        let path = library_path("invert", Path::new("target/debug"));
        let expected = PathBuf::from("target/debug")
            .join(library_filename("invert", Os::current()));
        assert_eq!(path, expected);
    }

    #[test]
    fn load_nonexistent_plugin_returns_error() {
        let result = PluginLoader::load(
            "nonexistent_plugin_xyz",
            Path::new("target/debug"),
        );
        assert!(result.is_err());
    }
}
