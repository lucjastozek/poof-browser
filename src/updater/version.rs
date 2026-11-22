//! Release versions: `1.4.2` or `v1.4.2`. Pre-releases (`1.4.2-beta`) are never offered.

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version([u64; 3]);

impl Version {
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().trim_start_matches('v');
        let mut parts = [0u64; 3];
        let mut count = 0;
        for piece in text.split('.') {
            if count == 3 {
                return None;
            }
            parts[count] = piece.parse().ok()?;
            count += 1;
        }
        (count > 0).then_some(Self(parts))
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0[0], self.0[1], self.0[2])
    }
}

#[cfg(test)]
mod tests {
    use super::Version;

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn compares_numerically_not_as_text() {
        assert!(v("1.10.0") > v("1.9.9"));
        assert!(v("v2.0") > v("1.99.99"));
        assert_eq!(v("1.2"), v("1.2.0"));
    }

    #[test]
    fn rejects_prereleases_and_junk() {
        for bad in ["", "v", "1.2.3-beta", "1.2.3.4", "1..3", "latest"] {
            assert!(Version::parse(bad).is_none(), "{bad}");
        }
    }
}
