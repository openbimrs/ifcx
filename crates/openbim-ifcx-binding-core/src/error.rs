//! Errors raised by every binding.
//!
//! Each error carries a stable machine-readable [`BindingError::code`] and a
//! human message. The codes are shared by all hosts, so a JavaScript
//! `err.code` and a Python `err.code` name the same failure the same way.
//!
//! Every code is therefore part of the public contract of both bindings: a
//! code may be added, never renamed or reused for a different failure. The
//! `tests` module below pins the released set.

use std::fmt;

/// Why a binding call failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingError {
    /// The input is not an IFCX file: invalid JSON, or JSON that does not
    /// have the IFCX file shape.
    Read(String),
    /// The file could not be written as JSON.
    Write(String),
    /// The imports of a layer could not be resolved: a missing layer, an
    /// import cycle, an unreadable imported layer, or an `integrity`
    /// mismatch.
    Layer(String),
    /// The layers do not compose: a reference cycle, or a reference to a
    /// node that no layer defines.
    Compose(String),
    /// A host argument was not what the call expects, such as an empty layer
    /// list or a value of the wrong type.
    InvalidArgument(String),
    /// The render scene could not be written as GLB, for example because
    /// the file would exceed 4 GiB.
    Glb(String),
}

impl BindingError {
    /// Stable code for programmatic handling.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Read(_) => "read",
            Self::Write(_) => "write",
            Self::Layer(_) => "layer",
            Self::Compose(_) => "compose",
            Self::InvalidArgument(_) => "invalid-argument",
            Self::Glb(_) => "glb",
        }
    }
}

impl fmt::Display for BindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(detail) | Self::Layer(detail) | Self::Compose(detail) => f.write_str(detail),
            Self::Write(detail) => write!(f, "cannot write IFCX: {detail}"),
            Self::InvalidArgument(detail) => write!(f, "invalid argument: {detail}"),
            Self::Glb(detail) => write!(f, "cannot write GLB: {detail}"),
        }
    }
}

impl std::error::Error for BindingError {}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::BindingError;

    /// Released codes in declaration order. Append a new code; never edit or
    /// remove one, because hosts match on these strings.
    const RELEASED_CODES: &[&str] = &[
        "read",
        "write",
        "layer",
        "compose",
        "invalid-argument",
        "glb",
    ];

    /// One value of every variant, in declaration order.
    fn one_of_each() -> Vec<BindingError> {
        let all = vec![
            BindingError::Read(String::new()),
            BindingError::Write(String::new()),
            BindingError::Layer(String::new()),
            BindingError::Compose(String::new()),
            BindingError::InvalidArgument(String::new()),
            BindingError::Glb(String::new()),
        ];
        // Exhaustive on purpose: a new variant does not compile until it is
        // listed above, so its code cannot escape the snapshot.
        for error in &all {
            match error {
                BindingError::Read(_)
                | BindingError::Write(_)
                | BindingError::Layer(_)
                | BindingError::Compose(_)
                | BindingError::InvalidArgument(_)
                | BindingError::Glb(_) => {}
            }
        }
        all
    }

    #[test]
    fn released_codes_are_never_renamed_or_reused() {
        let codes: Vec<&str> = one_of_each().iter().map(BindingError::code).collect();
        assert!(
            codes.len() >= RELEASED_CODES.len(),
            "a BindingError variant was removed: {codes:?}"
        );
        assert_eq!(
            &codes[..RELEASED_CODES.len()],
            RELEASED_CODES,
            "a released BindingError code changed; add a new code instead"
        );
        let unique: BTreeSet<&str> = codes.iter().copied().collect();
        assert_eq!(
            unique.len(),
            codes.len(),
            "two variants share a code: {codes:?}"
        );
    }
}
