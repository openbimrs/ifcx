//! `integrity` values on imports; see [`check_integrity`].

use std::fmt;

/// Why an `integrity` value rejected a layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrityError {
    /// The digest of the layer matches no token of the strongest algorithm.
    Mismatch {
        expected: String,
        /// The layer's digest as `<alg>-<base64>`.
        actual: String,
    },
    /// The value is empty or a token is not `<alg>-<base64>`.
    Malformed { value: String },
    /// No token uses a supported algorithm.
    Unsupported { value: String },
    /// The crate was built without the `integrity` feature, so the value
    /// cannot be checked.
    Unavailable { value: String },
}

impl fmt::Display for IntegrityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mismatch { expected, actual } => {
                write!(
                    f,
                    "integrity mismatch: expected {expected:?}, got {actual:?}"
                )
            }
            Self::Malformed { value } => write!(f, "malformed integrity value {value:?}"),
            Self::Unsupported { value } => write!(
                f,
                "integrity value {value:?} uses no supported algorithm (sha256, sha384, sha512)"
            ),
            Self::Unavailable { value } => write!(
                f,
                "cannot check integrity value {value:?}: built without the `integrity` feature"
            ),
        }
    }
}

impl std::error::Error for IntegrityError {}

/// Checks `bytes` against an import's `integrity` value.
///
/// The `ifcx_alpha` draft declares `integrity?: string` on an import and
/// defines no format; upstream's providers leave checking as a TODO and no
/// upstream example sets the field. This crate reads it as W3C Subresource
/// Integrity metadata: one or more whitespace-separated `<alg>-<base64>`
/// tokens, where `<alg>` is `sha256`, `sha384`, or `sha512` and `<base64>` is
/// the standard, padded base64 digest of the layer's bytes exactly as the
/// resolver returns them. As in SRI, only tokens of the strongest algorithm
/// present count, and one matching token suffices; `?options` after a digest
/// are ignored.
///
/// Unlike SRI, which treats unusable metadata as absent, a value that is
/// empty, malformed, or names only unknown algorithms is an error, so a layer
/// is never loaded unchecked when its importer asked for a check. Built
/// without the `integrity` feature, every value fails with
/// [`IntegrityError::Unavailable`] for the same reason.
///
/// ```
/// use openbim_ifcx::layers::check_integrity;
///
/// // sha256 of the empty input.
/// let value = "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=";
/// # #[cfg(feature = "integrity")]
/// assert!(check_integrity(value, b"").is_ok());
/// assert!(check_integrity(value, b"x").is_err());
/// ```
pub fn check_integrity(value: &str, bytes: &[u8]) -> Result<(), IntegrityError> {
    imp::check(value, bytes)
}

#[cfg(feature = "integrity")]
mod imp {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine as _;
    use sha2::{Digest, Sha256, Sha384, Sha512};

    use super::IntegrityError;

    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum Algorithm {
        Sha256,
        Sha384,
        Sha512,
    }

    impl Algorithm {
        fn parse(name: &str) -> Option<Self> {
            match name {
                "sha256" => Some(Self::Sha256),
                "sha384" => Some(Self::Sha384),
                "sha512" => Some(Self::Sha512),
                _ => None,
            }
        }

        fn name(self) -> &'static str {
            match self {
                Self::Sha256 => "sha256",
                Self::Sha384 => "sha384",
                Self::Sha512 => "sha512",
            }
        }

        fn digest(self, bytes: &[u8]) -> Vec<u8> {
            match self {
                Self::Sha256 => Sha256::digest(bytes).to_vec(),
                Self::Sha384 => Sha384::digest(bytes).to_vec(),
                Self::Sha512 => Sha512::digest(bytes).to_vec(),
            }
        }
    }

    pub(super) fn check(value: &str, bytes: &[u8]) -> Result<(), IntegrityError> {
        let malformed = || IntegrityError::Malformed {
            value: value.to_owned(),
        };
        let mut tokens = Vec::new();
        for token in value.split_ascii_whitespace() {
            let (name, rest) = token.split_once('-').ok_or_else(malformed)?;
            let digest = rest.split_once('?').map_or(rest, |(digest, _)| digest);
            let digest = STANDARD.decode(digest).map_err(|_| malformed())?;
            if let Some(algorithm) = Algorithm::parse(name) {
                tokens.push((algorithm, digest));
            }
        }
        if value.split_ascii_whitespace().next().is_none() {
            return Err(malformed());
        }
        let Some(strongest) = tokens.iter().map(|(a, _)| *a).max() else {
            return Err(IntegrityError::Unsupported {
                value: value.to_owned(),
            });
        };
        let actual = strongest.digest(bytes);
        if tokens
            .iter()
            .any(|(a, digest)| *a == strongest && *digest == actual)
        {
            Ok(())
        } else {
            Err(IntegrityError::Mismatch {
                expected: value.to_owned(),
                actual: format!("{}-{}", strongest.name(), STANDARD.encode(actual)),
            })
        }
    }
}

#[cfg(not(feature = "integrity"))]
mod imp {
    use super::IntegrityError;

    pub(super) fn check(value: &str, _bytes: &[u8]) -> Result<(), IntegrityError> {
        Err(IntegrityError::Unavailable {
            value: value.to_owned(),
        })
    }
}

#[cfg(all(test, feature = "integrity"))]
mod tests {
    use super::{check_integrity, IntegrityError};

    // Digests of b"abc".
    const SHA256: &str = "sha256-ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0=";
    const SHA512: &str =
        "sha512-3a81oZNherrMQXNJriBBMRLm+k6JqX6iCp7u5ktV05ohkpkqJ0/BqDa6PCOj/uu9RU1EI2Q86A4qmslPpUyknw==";

    #[test]
    fn accepts_matching_digests() {
        assert_eq!(check_integrity(SHA256, b"abc"), Ok(()));
        assert_eq!(check_integrity(SHA512, b"abc"), Ok(()));
        assert_eq!(check_integrity(&format!("{SHA256}?opt"), b"abc"), Ok(()));
        assert_eq!(check_integrity(&format!(" {SHA256}\n"), b"abc"), Ok(()));
        assert_eq!(
            check_integrity(&format!("md5-AAAA {SHA256}"), b"abc"),
            Ok(())
        );
    }

    #[test]
    fn strongest_algorithm_decides() {
        let wrong512 = format!("sha512-{}", "A".repeat(86) + "==");
        let value = format!("{SHA256} {wrong512}");
        assert!(matches!(
            check_integrity(&value, b"abc"),
            Err(IntegrityError::Mismatch { actual, .. }) if actual == SHA512
        ));
    }

    #[test]
    fn rejects_bad_values() {
        assert!(matches!(
            check_integrity(SHA256, b"abd"),
            Err(IntegrityError::Mismatch { actual, .. }) if actual.starts_with("sha256-")
        ));
        for value in ["", "  ", "sha256", "sha256-not base64!"] {
            assert!(
                matches!(
                    check_integrity(value, b"abc"),
                    Err(IntegrityError::Malformed { .. })
                ),
                "{value:?}"
            );
        }
        assert!(matches!(
            check_integrity("md5-AAAA", b"abc"),
            Err(IntegrityError::Unsupported { .. })
        ));
    }
}
