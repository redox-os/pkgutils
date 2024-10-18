use std::collections::HashMap;

use serde_derive::{Deserialize, Serialize};
use toml::{self, from_str, to_string};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Packages {
    pub installed: HashMap<String, String>, // package -> checksum
    pub protected: Vec<String>,

    pub files: HashMap<String, Vec<String>>, // package -> paths
}

impl Packages {
    pub fn from_toml(text: &str) -> Result<Self, toml::de::Error> {
        from_str(text)
    }

    pub fn to_toml(&self) -> String {
        // to_string *should* be safe to unwrap for this struct
        to_string(self).unwrap()
    }
}
