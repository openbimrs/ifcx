//! Reading and writing IFCX files as JSON.

use std::fmt;
use std::io::{Read, Write};

use serde_json::error::Category;

use crate::model::IfcxFile;

/// Why a file could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadErrorKind {
    /// Reading the underlying stream failed.
    Io,
    /// The input is not valid JSON.
    Syntax,
    /// The input is JSON but does not match the IFCX file shape.
    Data,
    /// The input ended early.
    Eof,
}

/// A file that could not be read, with the 1-based location of the problem.
#[derive(Debug)]
pub struct ReadError {
    kind: ReadErrorKind,
    line: usize,
    column: usize,
    source: serde_json::Error,
}

impl ReadError {
    pub fn kind(&self) -> ReadErrorKind {
        self.kind
    }

    /// 1-based line of the problem, or 0 for an I/O error.
    pub fn line(&self) -> usize {
        self.line
    }

    /// 1-based column of the problem, or 0 for an I/O error.
    pub fn column(&self) -> usize {
        self.column
    }
}

impl From<serde_json::Error> for ReadError {
    fn from(source: serde_json::Error) -> Self {
        let kind = match source.classify() {
            Category::Io => ReadErrorKind::Io,
            Category::Syntax => ReadErrorKind::Syntax,
            Category::Data => ReadErrorKind::Data,
            Category::Eof => ReadErrorKind::Eof,
        };
        Self {
            kind,
            line: source.line(),
            column: source.column(),
            source,
        }
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid IFCX file: {}", self.source)
    }
}

impl std::error::Error for ReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// A file that could not be written.
#[derive(Debug)]
pub struct WriteError(serde_json::Error);

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "could not write IFCX file: {}", self.0)
    }
}

impl std::error::Error for WriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl IfcxFile {
    pub fn from_json_str(input: &str) -> Result<Self, ReadError> {
        Ok(serde_json::from_str(input)?)
    }

    pub fn from_json_slice(input: &[u8]) -> Result<Self, ReadError> {
        Ok(serde_json::from_slice(input)?)
    }

    /// Reads from a stream. Wrap unbuffered sources in a `BufReader`.
    pub fn from_json_reader(reader: impl Read) -> Result<Self, ReadError> {
        Ok(serde_json::from_reader(reader)?)
    }

    pub fn to_json_string(&self) -> Result<String, WriteError> {
        serde_json::to_string(self).map_err(WriteError)
    }

    pub fn to_json_string_pretty(&self) -> Result<String, WriteError> {
        serde_json::to_string_pretty(self).map_err(WriteError)
    }

    pub fn to_json_writer(&self, writer: impl Write) -> Result<(), WriteError> {
        serde_json::to_writer(writer, self).map_err(WriteError)
    }
}
