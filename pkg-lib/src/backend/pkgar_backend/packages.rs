use serde_derive::{Deserialize, Serialize};

use crate::PackageName;

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct Packages {
    pub protected: Vec<PackageName>,
}

impl Packages {
    pub fn from_toml(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }

    pub fn to_toml(&self) -> String {
        // to_string *should* be safe to unwrap for this struct
        toml::to_string(self).unwrap()
    }
}
