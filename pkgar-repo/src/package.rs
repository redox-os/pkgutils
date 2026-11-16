use crate::{Error, cursor::BufCacheReader, reader::PackageUrlReader};
use pkgar_core::{Header, PackageSrc, PublicKey, Zeroable};

pub type PackageUrlCache<'a> = BufCacheReader<PackageUrlReader<'a>>;

pub struct PackageUrl<'a> {
    client: &'a ureq::Agent,
    url: String,
    header: Header,
    len: u64,
    cache: Option<PackageUrlCache<'a>>,
}

impl<'a> PackageUrl<'a> {
    pub fn new(
        client: &'a ureq::Agent,
        url: String,
        public_key: &PublicKey,
    ) -> Result<Self, Error> {
        let head_resp = client
            .head(&url)
            .call()
            .map_err(|source| pkgar_keys::Error::Io {
                source: std::io::Error::new(std::io::ErrorKind::Other, source.to_string()),
                path: Some(url.clone().into()),
                context: "Fetching remote HEAD",
            })?;

        let actual_len: u64 = head_resp
            .headers()
            .get("Content-Length")
            .and_then(|s| s.to_str().ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let mut new = Self {
            client,
            url,
            // Need a blank header to construct the PackageFile, since we need to
            // use a method of PackageSrc in order to get the actual header...
            header: Header::zeroed(),
            len: actual_len,
            cache: None,
        };
        new.header = new.read_header(public_key)?;
        Ok(new)
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get new uncached reader
    pub fn get_reader(&self) -> PackageUrlReader<'a> {
        PackageUrlReader::new(self.client, &self.url, self.len)
    }

    pub fn take_reader(&mut self) -> Result<PackageUrlCache<'a>, Error> {
        Ok(self.cache.take().unwrap_or_else(|| {
            // create new one
            BufCacheReader::new(self.get_reader(), self.len, 128 * 1024)
        }))
    }

    pub fn restore_reader(&mut self, reader: PackageUrlCache<'a>) -> Result<(), Error> {
        self.cache = Some(reader);
        Ok(())
    }

    pub fn len(&self) -> u64 {
        self.len
    }
}

impl PackageSrc for PackageUrl<'_> {
    type Err = Error;

    fn header(&self) -> Header {
        self.header
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, Self::Err> {
        PackageUrlReader::new(self.client, &self.url, self.len).read_at(offset, buf)
    }
}
