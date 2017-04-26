use toml::{self, to_string, from_str};

#[derive(Serialize, Deserialize)]
pub struct PackageMeta {
    name: String,
    version: String,
    target: String,
}

impl PackageMeta {
    pub fn new(name: &str, version: &str, target: &str) -> PackageMeta {
        PackageMeta {
            name: name.to_string(),
            version: version.to_string(),
            target: target.to_string(),
        }
    }

    pub fn from_toml(text: &str) -> Result<Self, toml::de::Error> {
       from_str(text)
    }

    pub fn to_toml(&self) -> String {
        // to_string *should* be safe to unwrap for this struct
        to_string(self).unwrap()
    }
}
