use std::fs::File;
use std::path::Path;
use std::path::PathBuf;
use std::io;
use tar::Archive;

//use packagemeta::PackageMeta;

pub struct Package {
    archive: Archive<File>,
    path: PathBuf,
    //meta: Option<PackageMeta>,
}

impl Package {
    pub fn from_path<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut ar = Archive::new(File::open(&path)?);
        ar.set_preserve_permissions(true);
        Ok(Package{archive: ar, path: path.as_ref().to_path_buf()})
    }

    pub fn install(&mut self, dest: &str)-> io::Result<()> {
        self.archive.unpack(dest)?;
        Ok(())
    }

    pub fn list(&mut self) -> io::Result<()> {
        for i in self.archive.entries()? {
            println!("{}", i?.path()?.display());
        }
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
