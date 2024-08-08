use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::str::FromStr;
use std::{fmt::Display, ops::Deref};
use ulid::Ulid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct Eid(String);

impl Display for Eid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Eid {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Eid(s.to_string()))
    }
}

impl Deref for Eid {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<&str> for Eid {
    fn from(fr: &str) -> Self {
        Eid(fr.to_string())
    }
}

impl From<String> for Eid {
    fn from(fr: String) -> Self {
        Eid(fr)
    }
}

impl From<Eid> for String {
    fn from(fr: Eid) -> Self {
        fr.0
    }
}

impl Eid {
    #[inline]
    pub fn new() -> Eid {
        Eid(Ulid::new().to_string())
    }
}

impl Default for Eid {
    fn default() -> Self {
        Self::new()
    }
}
