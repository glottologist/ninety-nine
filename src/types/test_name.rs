use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Serialize};

/// Newtype wrapper for test names, preventing confusion with other string fields
/// like branch names, commit hashes, or error messages.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TestName(String);

impl TestName {
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Deref for TestName {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for TestName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TestName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for TestName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for TestName {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl PartialEq<str> for TestName {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for TestName {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}
