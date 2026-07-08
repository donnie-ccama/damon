use std::fmt;

/// Kebab-case identifier: `^[a-z0-9][a-z0-9-]{0,31}$`. Excludes `_` so tmux
/// session names (underscore-joined) parse unambiguously.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct Slug(String);

#[derive(thiserror::Error, Debug)]
#[error("invalid slug {0:?} (want ^[a-z0-9][a-z0-9-]{{0,31}}$)")]
pub struct SlugError(pub String);

impl Slug {
    pub fn derive(name: &str) -> Result<Self, SlugError> {
        let mut out = String::new();
        let mut pending_dash = false;
        for c in name.chars().flat_map(|c| c.to_lowercase()) {
            if c.is_ascii_alphanumeric() {
                if pending_dash && !out.is_empty() {
                    out.push('-');
                }
                out.push(c);
                pending_dash = false;
            } else {
                pending_dash = true;
            }
        }
        let truncated: String = out.chars().take(32).collect();
        Self::parse(truncated.trim_end_matches('-')).map_err(|_| SlugError(name.to_string()))
    }

    pub fn parse(s: &str) -> Result<Self, SlugError> {
        let first_ok = s
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
        let rest_ok = s
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if first_ok && rest_ok && s.len() <= 32 {
            Ok(Slug(s.to_string()))
        } else {
            Err(SlugError(s.to_string()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Slug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for Slug {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Slug::parse(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_kebab_case() {
        assert_eq!(
            Slug::derive("Newsletter Team").unwrap().as_str(),
            "newsletter-team"
        );
        assert_eq!(Slug::derive("Scout").unwrap().as_str(), "scout");
        assert_eq!(Slug::derive("a  b!!c").unwrap().as_str(), "a-b-c");
    }

    #[test]
    fn derive_truncates_to_32_and_trims_dashes() {
        let s = Slug::derive(&format!("{}x", "a".repeat(40))).unwrap();
        assert_eq!(s.as_str().len(), 32);
        assert!(!s.as_str().ends_with('-'));
    }

    #[test]
    fn derive_rejects_empty_and_symbol_only() {
        assert!(Slug::derive("").is_err());
        assert!(Slug::derive("!!!").is_err());
        assert!(Slug::derive("---").is_err());
    }

    #[test]
    fn parse_validates() {
        assert!(Slug::parse("scout").is_ok());
        assert!(Slug::parse("scout-2").is_ok());
        assert!(Slug::parse("Scout").is_err()); // uppercase
        assert!(Slug::parse("bad_name").is_err()); // underscore reserved for session names
        assert!(Slug::parse("-lead").is_err()); // must start alphanumeric
        assert!(Slug::parse(&"a".repeat(33)).is_err());
    }

    #[test]
    fn deserializes_with_validation() {
        assert!(serde_json::from_str::<Slug>("\"scout\"").is_ok());
        assert!(serde_json::from_str::<Slug>("\"Bad_Slug\"").is_err());
    }
}
