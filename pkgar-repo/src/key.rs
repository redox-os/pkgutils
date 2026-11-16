use std::path::Path;

use pkgar_keys::PublicKeyFile;

use crate::Error;

pub struct PublicKeyRemote {}

impl PublicKeyRemote {
    pub fn download(client: &ureq::Agent, path: &str) -> Result<PublicKeyFile, Error> {
        let mut response = client.get(path).call()?;
        let content = response.body_mut().read_to_string()?;
        PublicKeyFile::from_str(&content).map_err(pkgar_keys::Error::into)
    }
    pub fn download_or_read(client: &ureq::Agent, path: &Path) -> Result<PublicKeyFile, Error> {
        if let Some(path) = path.to_str()
            && crate::is_remote(path)
        {
            Self::download(client, path)
        } else {
            PublicKeyFile::open(Path::new(path)).map_err(pkgar_keys::Error::into)
        }
    }
}
