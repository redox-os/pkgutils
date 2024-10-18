use serde_derive::{Deserialize, Serialize};
use toml::{self, from_str, to_string};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub target: String,
    //pub summary: String,
    //pub description: String,
    pub depends: Vec<String>,
}

/*impl Ord for Package {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name.cmp(&other.name)
    }
}*/

impl Package {
    pub fn from_toml(text: &str) -> Result<Self, toml::de::Error> {
        from_str(text)
    }

    #[allow(dead_code)]
    pub fn to_toml(&self) -> String {
        // to_string *should* be safe to unwrap for this struct
        // use error handeling callbacks for this
        to_string(self).unwrap()
    }
}

#[derive(Debug)]
pub struct PackageInfo {
    pub installed: bool,
    pub version: String,
    pub target: String,

    pub download_size: String,
    // pub install_size: String,
    pub depends: Vec<String>,
}
