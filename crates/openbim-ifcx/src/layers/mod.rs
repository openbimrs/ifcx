//! Layer stacks: loading a main layer and, recursively, its `imports`.
//!
//! A [`LayerStackBuilder`] asks a caller-supplied [`LayerResolver`] for each
//! layer, loads every layer once, checks `integrity` where an import carries
//! it, and returns the layers as a [`LayerStack`] in the order upstream's
//! `IfcxLayerStackBuilder` (`buildingSMART/IFC5-development`,
//! `src/ifcx-core/layers/layer-stack.ts`, commit `1a63082`) uses. The crate
//! performs no network access. A filesystem resolver, [`FsResolver`], is
//! available behind the `fs` feature, and [`MemoryResolver`] serves layers
//! held in memory.
//!
//! # Layer order
//!
//! The main layer comes first. Then, for each layer, all of its imports that
//! are not loaded yet are claimed in the order written, and each claimed
//! import is followed by its own new imports before the next claimed one. For
//! `main` importing `a` and `b`, and `a` importing `c`:
//!
//! ```text
//! main, a, c, b
//! ```
//!
//! An import of a layer that is already in the stack is not loaded again and
//! does not move it.
//!
//! [`LayerStack::federate`] concatenates schemas and data in this order, as
//! upstream's `Federate` does, and [`flatten`](crate::flatten) lets later
//! opinions win. So, in
//! upstream's effective behaviour, **an imported layer overrides the layer
//! that imports it**, later imports override earlier ones, and nested imports
//! override their importer too. The main layer has the lowest priority for
//! every path and attribute an import also sets. Nothing in the `ifcx_alpha`
//! draft text states a priority between a layer and its imports; this crate
//! reproduces the reference implementation and records the finding in
//! `docs/capabilities.md`.
//!
//! Upstream's builder appends a nested layer's subtree more than once (for
//! `main → a → c` it yields `main, c, a, c`). Because later opinions win, the
//! composed result is the same as for the list above, where each layer keeps
//! the position of its last occurrence; only the first-seen position of a
//! path or schema key can differ. This crate loads and lists each layer once.
//!
//! # Cycles
//!
//! Upstream silently skips an import of a layer that is already placed, so it
//! accepts import cycles. This crate rejects them with
//! [`LayerError::Cycle`] by default; [`LayerStackBuilder::allow_cycles`]
//! restores upstream's behaviour.
//!
//! ```
//! use openbim_ifcx::layers::{LayerStackBuilder, MemoryResolver};
//!
//! let layer = |id: &str, imports: &str, value: &str| format!(r#"{{
//!     "header": {{"id": "{id}", "ifcxVersion": "ifcx_alpha", "dataVersion": "1.0.0",
//!                "author": "someone", "timestamp": "2026-10-03"}},
//!     "imports": [{imports}],
//!     "schemas": {{}},
//!     "data": [{{"path": "p1", "attributes": {{"demo::value": "{value}"}}}}]
//! }}"#);
//!
//! let mut resolver = MemoryResolver::new();
//! resolver.insert("main", layer("main", r#"{"uri": "base"}"#, "main"));
//! resolver.insert("base", layer("base", "", "base"));
//!
//! let stack = LayerStackBuilder::new(resolver).build("main")?;
//! assert_eq!(stack.keys().collect::<Vec<_>>(), ["main", "base"]);
//!
//! // Data stays in stack order; the import's opinion comes last and wins.
//! let federated = stack.federate();
//! assert_eq!(federated.data.len(), 2);
//! assert_eq!(federated.data[1].attributes.as_ref().unwrap()["demo::value"], "base");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#[cfg(feature = "fs")]
mod fs;
mod integrity;
mod memory;

use std::collections::HashMap;
use std::fmt;

use indexmap::IndexMap;

use crate::json::ReadError;
use crate::model::{IfcxFile, ImportNode};
use crate::validate::ValidationReport;

#[cfg(feature = "fs")]
pub use fs::{FsError, FsResolver};
pub use integrity::{check_integrity, IntegrityError};
pub use memory::MemoryResolver;

/// Finds and loads layers for a [`LayerStackBuilder`].
///
/// Resolution happens in two steps so that a layer imported from several
/// places loads once: [`key`](Self::key) turns an import `uri` into a key that
/// identifies the layer, and [`load`](Self::load) returns the bytes for a key
/// that is not loaded yet. Integrity is checked against exactly these bytes.
pub trait LayerResolver {
    /// Failure other than a missing layer, such as an I/O error.
    type Error;

    /// Identifies the layer that `uri` names. `importer` is the key of the
    /// layer whose `imports` list `uri`, or `None` for the main layer, so
    /// relative references can resolve against it.
    ///
    /// The default uses `uri` unchanged, as upstream's providers do.
    fn key(&mut self, uri: &str, importer: Option<&str>) -> Result<String, Self::Error> {
        let _ = importer;
        Ok(uri.to_owned())
    }

    /// Returns the bytes of the layer with this key, or `Ok(None)` when there
    /// is no such layer.
    fn load(&mut self, key: &str) -> Result<Option<Vec<u8>>, Self::Error>;
}

impl<R: LayerResolver + ?Sized> LayerResolver for &mut R {
    type Error = R::Error;

    fn key(&mut self, uri: &str, importer: Option<&str>) -> Result<String, Self::Error> {
        (**self).key(uri, importer)
    }

    fn load(&mut self, key: &str) -> Result<Option<Vec<u8>>, Self::Error> {
        (**self).load(key)
    }
}

/// Why a layer stack could not be built. `E` is the resolver's error type.
#[derive(Debug)]
pub enum LayerError<E> {
    /// The resolver has no layer for this import.
    Missing {
        uri: String,
        /// Key of the importing layer; `None` for the main layer.
        importer: Option<String>,
    },
    /// The resolver failed.
    Resolver {
        uri: String,
        importer: Option<String>,
        source: E,
    },
    /// A loaded layer is not a valid IFCX file.
    Read { key: String, source: ReadError },
    /// The bytes of an imported layer do not satisfy the import's `integrity`,
    /// or the value cannot be checked.
    Integrity {
        uri: String,
        importer: String,
        source: IntegrityError,
    },
    /// Layers import each other in a loop. `chain` lists the keys along the
    /// loop and ends with the key it starts with.
    Cycle { chain: Vec<String> },
}

impl<E: fmt::Display> fmt::Display for LayerError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let from = |importer: &Option<String>| match importer {
            Some(key) => format!(" imported by {key:?}"),
            None => String::new(),
        };
        match self {
            Self::Missing { uri, importer } => {
                write!(f, "layer {uri:?}{} not found", from(importer))
            }
            Self::Resolver {
                uri,
                importer,
                source,
            } => write!(
                f,
                "could not resolve layer {uri:?}{}: {source}",
                from(importer)
            ),
            Self::Read { key, source } => write!(f, "layer {key:?}: {source}"),
            Self::Integrity {
                uri,
                importer,
                source,
            } => write!(f, "layer {uri:?} imported by {importer:?}: {source}"),
            Self::Cycle { chain } => write!(f, "import cycle: {}", chain.join(" -> ")),
        }
    }
}

impl<E> std::error::Error for LayerError<E>
where
    E: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resolver { source, .. } => Some(source),
            Self::Read { source, .. } => Some(source),
            Self::Integrity { source, .. } => Some(source),
            Self::Missing { .. } | Self::Cycle { .. } => None,
        }
    }
}

/// One loaded layer of a [`LayerStack`].
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    key: String,
    file: IfcxFile,
}

impl Layer {
    /// The key the resolver gave this layer.
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn file(&self) -> &IfcxFile {
        &self.file
    }

    pub fn into_file(self) -> IfcxFile {
        self.file
    }
}

/// A main layer and everything it imports, each once, in upstream order.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerStack {
    layers: Vec<Layer>,
}

impl LayerStack {
    /// The layer the stack was built from.
    pub fn main(&self) -> &Layer {
        &self.layers[0]
    }

    /// All layers, main layer first. See the [module docs](self) for the order.
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// Layer keys in stack order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.layers.iter().map(Layer::key)
    }

    pub fn into_layers(self) -> Vec<Layer> {
        self.layers
    }

    /// Merges the stack into one file, as upstream's `Federate` does before
    /// composing: the main layer's header, no imports, schemas from every
    /// layer, and the data of every layer in stack order.
    ///
    /// A schema id defined by several layers keeps the position where it first
    /// appears and takes the value of the last layer. Data nodes are not
    /// merged; several nodes may share a path, and [`flatten`](crate::flatten)
    /// applies them in order so that later opinions win.
    pub fn federate(&self) -> IfcxFile {
        federate(self.layers.iter().map(Layer::file)).expect("a layer stack is never empty")
    }

    /// [`federate`](Self::federate) for a stack that is not needed
    /// afterwards: the layers' schemas and data move into the result instead
    /// of being copied. Pass its `data` to [`flatten_owned`] to keep
    /// composition free of copies too.
    ///
    /// [`flatten_owned`]: crate::flatten_owned
    pub fn into_federated(self) -> IfcxFile {
        federate_owned(self.layers.into_iter().map(Layer::into_file))
            .expect("a layer stack is never empty")
    }

    /// Checks the attributes of the whole stack: the schemas of every layer,
    /// merged as [`federate`](Self::federate) does, against the attributes of
    /// every layer's data, merged per path as [`flatten`](crate::flatten)
    /// does. This is [`IfcxFile::validate`] of the federated file, without
    /// copying it.
    ///
    /// So an attribute whose schema lives only in an imported file validates,
    /// and only the value that wins for a path is checked.
    ///
    /// ```
    /// use openbim_ifcx::layers::{LayerStackBuilder, MemoryResolver};
    ///
    /// let header = r#""header": {"id": "x", "ifcxVersion": "ifcx_alpha", "dataVersion": "1.0.0",
    ///                            "author": "someone", "timestamp": "2026-10-03"}"#;
    /// let mut resolver = MemoryResolver::new();
    /// resolver.insert("main", format!(r#"{{{header}, "imports": [{{"uri": "schemas"}}],
    ///     "schemas": {{}}, "data": [{{"path": "w", "attributes": {{"demo::height": 2.5}}}}]}}"#));
    /// resolver.insert("schemas", format!(r#"{{{header}, "imports": [], "data": [],
    ///     "schemas": {{"demo::height": {{"value": {{"dataType": "Real"}}}}}}}}"#));
    ///
    /// let stack = LayerStackBuilder::new(resolver).build("main")?;
    /// // The main layer alone does not know `demo::height`; its stack does.
    /// assert!(stack.main().file().validate().is_err());
    /// assert!(stack.validate().is_ok());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn validate(&self) -> Result<(), ValidationReport> {
        crate::validate::validate_files(self.layers.iter().map(Layer::file)).into_result()
    }
}

/// Merges `files` in the order given, as upstream's `Federate` does: the first
/// file's header, no imports, schemas from every file (a repeated id keeps its
/// first position and takes the last value), and every data node in order.
/// Top-level fields this crate does not model are not carried over.
///
/// Returns `None` for no files. [`federate_owned`] does the same for files
/// that are not needed afterwards, without copying them.
pub fn federate<'a>(files: impl IntoIterator<Item = &'a IfcxFile>) -> Option<IfcxFile> {
    let mut files = files.into_iter().peekable();
    let header = files.peek()?.header.clone();
    let mut schemas = IndexMap::new();
    let mut data = Vec::new();
    for file in files {
        for (id, schema) in &file.schemas {
            schemas.insert(id.clone(), schema.clone());
        }
        data.extend(file.data.iter().cloned());
    }
    Some(IfcxFile {
        header,
        imports: Vec::new(),
        schemas,
        data,
        extra: Default::default(),
    })
}

/// [`federate`] for files that are not needed afterwards: their schemas and
/// data move into the result instead of being copied. The result is the same.
///
/// Returns `None` for no files.
pub fn federate_owned(files: impl IntoIterator<Item = IfcxFile>) -> Option<IfcxFile> {
    let mut files = files.into_iter();
    let first = files.next()?;
    let header = first.header;
    let mut schemas = first.schemas;
    let mut data = first.data;
    for file in files {
        schemas.extend(file.schemas);
        data.extend(file.data);
    }
    Some(IfcxFile {
        header,
        imports: Vec::new(),
        schemas,
        data,
        extra: Default::default(),
    })
}

/// Builds a [`LayerStack`] from a main layer through a [`LayerResolver`].
#[derive(Debug)]
pub struct LayerStackBuilder<R> {
    resolver: R,
    allow_cycles: bool,
}

impl<R: LayerResolver> LayerStackBuilder<R> {
    pub fn new(resolver: R) -> Self {
        Self {
            resolver,
            allow_cycles: false,
        }
    }

    /// Accept import cycles and skip the import that closes one, as upstream
    /// does. Off by default, so cycles are a [`LayerError::Cycle`].
    pub fn allow_cycles(mut self, allow: bool) -> Self {
        self.allow_cycles = allow;
        self
    }

    pub fn resolver(&self) -> &R {
        &self.resolver
    }

    pub fn resolver_mut(&mut self) -> &mut R {
        &mut self.resolver
    }

    pub fn into_resolver(self) -> R {
        self.resolver
    }

    /// Loads the layer `main` names and, recursively, its imports.
    pub fn build(&mut self, main: &str) -> Result<LayerStack, LayerError<R::Error>> {
        let key = self
            .resolver
            .key(main, None)
            .map_err(|source| LayerError::Resolver {
                uri: main.to_owned(),
                importer: None,
                source,
            })?;
        let mut build = Build::new(&mut self.resolver);
        let root = build.load(main, None, key)?;
        build.order.push(root);
        build.satisfy(root)?;
        Self::finish(self.allow_cycles, build)
    }

    /// Loads several layers given weakest first, and recursively their
    /// imports, as if they were the `imports` of a main layer without data.
    /// This is how upstream's `ifcx compose` and viewer stack the files a
    /// user names, so the last one wins.
    ///
    /// The stack holds only the named layers and their imports, in the order
    /// [`build`](Self::build) would give them below that main layer: each
    /// named layer, followed by its own new imports, which override it. A
    /// layer named twice, or also imported, loads once. `uris` resolve with
    /// no importer, as `build`'s `main` does. At least one is needed; with
    /// none, the result is [`LayerError::Missing`] for an empty URI.
    ///
    /// ```
    /// use openbim_ifcx::layers::{LayerStackBuilder, MemoryResolver};
    ///
    /// let layer = |id: &str| format!(r#"{{
    ///     "header": {{"id": "{id}", "ifcxVersion": "ifcx_alpha", "dataVersion": "1.0.0",
    ///                "author": "someone", "timestamp": "2026-10-03"}},
    ///     "imports": [], "schemas": {{}},
    ///     "data": [{{"path": "p1", "attributes": {{"demo::value": "{id}"}}}}]
    /// }}"#);
    /// let mut resolver = MemoryResolver::new();
    /// resolver.insert("base", layer("base"));
    /// resolver.insert("overlay", layer("overlay"));
    ///
    /// let stack = LayerStackBuilder::new(resolver).build_all(["base", "overlay"])?;
    /// assert_eq!(stack.keys().collect::<Vec<_>>(), ["base", "overlay"]);
    /// assert_eq!(stack.main().key(), "base");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn build_all<I>(&mut self, uris: I) -> Result<LayerStack, LayerError<R::Error>>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let mut build = Build::new(&mut self.resolver);
        let mut pending = Vec::new();
        for uri in uris {
            let uri = uri.as_ref();
            let key = build
                .resolver
                .key(uri, None)
                .map_err(|source| LayerError::Resolver {
                    uri: uri.to_owned(),
                    importer: None,
                    source,
                })?;
            if !build.index.contains_key(&key) {
                pending.push(build.load(uri, None, key)?);
            }
        }
        if pending.is_empty() {
            return Err(LayerError::Missing {
                uri: String::new(),
                importer: None,
            });
        }
        for i in pending {
            build.order.push(i);
            build.satisfy(i)?;
        }
        Self::finish(self.allow_cycles, build)
    }

    fn finish(allow_cycles: bool, build: Build<'_, R>) -> Result<LayerStack, LayerError<R::Error>> {
        if !allow_cycles {
            if let Some(chain) = find_cycle(&build.edges) {
                let chain = chain
                    .into_iter()
                    .map(|i| build.layers[i].key.clone())
                    .collect();
                return Err(LayerError::Cycle { chain });
            }
        }
        let mut layers: Vec<Option<Layer>> = build.layers.into_iter().map(Some).collect();
        let layers = build
            .order
            .iter()
            .map(|&i| layers[i].take().expect("each layer is placed once"))
            .collect();
        Ok(LayerStack { layers })
    }
}

struct Build<'r, R> {
    resolver: &'r mut R,
    /// Layers in load order; indices below refer to this list.
    layers: Vec<Layer>,
    /// Layer indices in stack order.
    order: Vec<usize>,
    /// Raw bytes per layer index, kept to check integrity of later imports.
    bytes: HashMap<usize, Vec<u8>>,
    index: HashMap<String, usize>,
    /// Import edges by layer index, for cycle detection.
    edges: Vec<Vec<usize>>,
}

impl<'r, R: LayerResolver> Build<'r, R> {
    fn new(resolver: &'r mut R) -> Self {
        Self {
            resolver,
            layers: Vec::new(),
            bytes: HashMap::new(),
            index: HashMap::new(),
            edges: Vec::new(),
            order: Vec::new(),
        }
    }

    fn load(
        &mut self,
        uri: &str,
        importer: Option<&str>,
        key: String,
    ) -> Result<usize, LayerError<R::Error>> {
        let bytes = match self.resolver.load(&key) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => {
                return Err(LayerError::Missing {
                    uri: uri.to_owned(),
                    importer: importer.map(str::to_owned),
                })
            }
            Err(source) => {
                return Err(LayerError::Resolver {
                    uri: uri.to_owned(),
                    importer: importer.map(str::to_owned),
                    source,
                })
            }
        };
        let file = IfcxFile::from_json_slice(&bytes).map_err(|source| LayerError::Read {
            key: key.clone(),
            source,
        })?;
        let i = self.layers.len();
        self.index.insert(key.clone(), i);
        self.bytes.insert(i, bytes);
        self.layers.push(Layer { key, file });
        self.edges.push(Vec::new());
        Ok(i)
    }

    /// Upstream's `SatisfyDependencies`: claim every new import of `layer` in
    /// order, then descend into each claimed import in turn.
    fn satisfy(&mut self, layer: usize) -> Result<(), LayerError<R::Error>> {
        let imports: Vec<ImportNode> = self.layers[layer].file.imports.clone();
        let importer = self.layers[layer].key.clone();
        let mut pending = Vec::new();
        for import in &imports {
            let key = self
                .resolver
                .key(&import.uri, Some(&importer))
                .map_err(|source| LayerError::Resolver {
                    uri: import.uri.clone(),
                    importer: Some(importer.clone()),
                    source,
                })?;
            let target = match self.index.get(&key) {
                Some(&i) => i,
                None => {
                    let i = self.load(&import.uri, Some(&importer), key)?;
                    pending.push(i);
                    i
                }
            };
            self.edges[layer].push(target);
            if let Some(expected) = &import.integrity {
                check_integrity(expected, &self.bytes[&target]).map_err(|source| {
                    LayerError::Integrity {
                        uri: import.uri.clone(),
                        importer: importer.clone(),
                        source,
                    }
                })?;
            }
        }
        for i in pending {
            self.order.push(i);
            self.satisfy(i)?;
        }
        Ok(())
    }
}

/// Returns a loop in the import graph as layer indices, first index repeated
/// at the end, or `None`.
fn find_cycle(edges: &[Vec<usize>]) -> Option<Vec<usize>> {
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        New,
        Open,
        Done,
    }
    let mut state = vec![State::New; edges.len()];
    for start in 0..edges.len() {
        if state[start] != State::New {
            continue;
        }
        // Iterative depth-first search; `path` holds open nodes and the next
        // edge to follow from each.
        let mut path: Vec<(usize, usize)> = vec![(start, 0)];
        state[start] = State::Open;
        while let Some(&mut (node, ref mut next)) = path.last_mut() {
            if let Some(&target) = edges[node].get(*next) {
                *next += 1;
                match state[target] {
                    State::Open => {
                        let from = path.iter().position(|&(n, _)| n == target).unwrap();
                        let mut chain: Vec<usize> = path[from..].iter().map(|&(n, _)| n).collect();
                        chain.push(target);
                        return Some(chain);
                    }
                    State::New => {
                        state[target] = State::Open;
                        path.push((target, 0));
                    }
                    State::Done => {}
                }
            } else {
                state[node] = State::Done;
                path.pop();
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::find_cycle;

    #[test]
    fn finds_cycles() {
        assert_eq!(find_cycle(&[vec![1], vec![2], vec![]]), None);
        assert_eq!(find_cycle(&[vec![1, 2], vec![2], vec![]]), None);
        assert_eq!(find_cycle(&[vec![0]]), Some(vec![0, 0]));
        assert_eq!(
            find_cycle(&[vec![1], vec![2], vec![1]]),
            Some(vec![1, 2, 1])
        );
    }
}
