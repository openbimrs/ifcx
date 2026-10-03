use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use super::LayerResolver;

/// Loads layers from the local filesystem. Requires the `fs` feature.
///
/// An import `uri` is a file path. A relative path resolves against the
/// directory of the importing layer, and the main layer's against the current
/// directory. Keys are canonical paths where the file exists, so a layer
/// reached by different relative paths loads once.
///
/// URIs with a scheme (`scheme://...`) are rejected with
/// [`FsError::UnsupportedUri`] unless [`map_prefix`](Self::map_prefix) maps
/// them to a local directory, for example an offline mirror of `ifcx.dev`:
///
/// ```no_run
/// use openbim_ifcx::layers::{FsResolver, LayerStackBuilder};
///
/// let resolver = FsResolver::new().map_prefix("https://ifcx.dev/", "mirror/ifcx.dev");
/// let stack = LayerStackBuilder::new(resolver).build("model.ifcx")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Default)]
pub struct FsResolver {
    prefixes: Vec<(String, PathBuf)>,
}

impl FsResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolves URIs starting with `prefix` to the rest of the URI below
    /// `dir`. The first matching prefix in insertion order applies. The rest
    /// is used as written; percent-escapes are not decoded.
    pub fn map_prefix(mut self, prefix: impl Into<String>, dir: impl Into<PathBuf>) -> Self {
        self.prefixes.push((prefix.into(), dir.into()));
        self
    }

    fn path(&self, uri: &str, importer: Option<&str>) -> Result<PathBuf, FsError> {
        for (prefix, dir) in &self.prefixes {
            if let Some(rest) = uri.strip_prefix(prefix.as_str()) {
                return Ok(dir.join(rest.trim_start_matches('/')));
            }
        }
        if uri.contains("://") {
            return Err(FsError::UnsupportedUri(uri.to_owned()));
        }
        let path = Path::new(uri);
        Ok(match importer.and_then(|key| Path::new(key).parent()) {
            Some(dir) if path.is_relative() => dir.join(path),
            _ => path.to_path_buf(),
        })
    }
}

impl LayerResolver for FsResolver {
    type Error = FsError;

    fn key(&mut self, uri: &str, importer: Option<&str>) -> Result<String, FsError> {
        let path = self.path(uri, importer)?;
        // A missing file keeps its joined path; `load` then reports it missing.
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        path.into_os_string()
            .into_string()
            .map_err(|path| FsError::NonUtf8Path(path.into()))
    }

    fn load(&mut self, key: &str) -> Result<Option<Vec<u8>>, FsError> {
        match std::fs::read(key) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(FsError::Io {
                path: key.into(),
                source,
            }),
        }
    }
}

/// Why [`FsResolver`] could not load a layer.
#[derive(Debug)]
pub enum FsError {
    /// The URI has a scheme and no [`FsResolver::map_prefix`] mapping.
    UnsupportedUri(String),
    /// Layer keys are strings, and this path is not valid UTF-8.
    NonUtf8Path(PathBuf),
    Io {
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedUri(uri) => write!(f, "no local mapping for URI {uri:?}"),
            Self::NonUtf8Path(path) => write!(f, "path {} is not valid UTF-8", path.display()),
            Self::Io { path, source } => write!(f, "reading {}: {source}", path.display()),
        }
    }
}

impl std::error::Error for FsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::UnsupportedUri(_) | Self::NonUtf8Path(_) => None,
        }
    }
}
