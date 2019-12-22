use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{self, Write};
use std::fmt::{self, Display, Formatter};
use std::ffi::OsStr;
use std::process::Command;
use std::ops::Deref;
use std::env;

use ion_shell::{Shell, IonError, Value};
use ion_shell::builtins::Status;

use ::{PackageMeta, Repo, download};

enum Source {
    Git(String, Option<String>),
    Tar(String)
}

#[derive(Debug)]
pub enum CookError {
    IO(io::Error),
    Ion(IonError),
    MissingVar(String),
    NonZero(String, i32),
}

impl From<io::Error> for CookError {
    fn from(err: io::Error) -> CookError {
        CookError::IO(err)
    }
}

impl From<IonError> for CookError {
    fn from(err: IonError) -> CookError {
        CookError::Ion(err)
    }
}

impl Display for CookError {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        match *self {
            CookError::IO(ref e) => e.fmt(fmt),
            CookError::Ion(ref e) => e.fmt(fmt),
            CookError::MissingVar(ref var) =>
                fmt.write_fmt(format_args!("Recipe missing '{}' variable", var)),
            CookError::NonZero(ref func, status) =>
                fmt.write_fmt(format_args!("{} returned non-zero status '{}'", func, status)),
        }
    }
}

type Result<T> = std::result::Result<T, CookError>;

pub struct Recipe<'a> {
    target: String,
    shell: Shell<'a>,
    #[allow(dead_code)]
    debug: bool,
    cookbook_dir: PathBuf,
}

// try! on IOError, except NotFound is okay (for removing files/dirs)
macro_rules! try_ifexist {
    ( $x:expr ) => {
        if let Err(err) = $x {
            if err.kind() != io::ErrorKind::NotFound {
                return Err(err.into());
            }
        }
    }
}

// Return Err on non-zero status
macro_rules! try_process_status {
    ( $cmd:expr, $status:expr) => {
        {
            let status = $status.code().unwrap_or(0); // ?

            if status != 0 {
                return Err(CookError::NonZero($cmd.to_string(), status));
            }
        }
    }
}

/// Call an Ion function in the source/ directory
fn call_func_src(shell: &mut Shell, func: &str, args: &[&str]) -> Result<()> {
    let prev_dir = env::current_dir()?;
    env::set_current_dir("source")?;

    let mut args_vec = vec![func];
    args_vec.extend(args);
    if let Some(Value::Function(function)) = shell.variables().get(func) {
        let res = match shell.execute_function(&function.clone(), &args_vec) {
            Err(e) => Err(e.into()),
            Ok(Status::SUCCESS) => Ok(()),
            Ok(status) => Err(CookError::NonZero(func.to_string(), status.as_os_code())),
        };

        env::set_current_dir(&prev_dir)?;
        res
    } else {
        Ok(()) // TODO
    }

}

impl<'a> Recipe<'a> {
    pub fn new<T: AsRef<Path>>(target: String, cookbook_dir: T, package: &str, debug: bool) -> Result<Recipe> {
        let mut shell = Shell::new();
        //XXX shell.flags |= ERR_EXIT;
        let variables = shell.variables_mut();
        variables.set("DEBUG", if debug { "1".to_string() } else { "0".to_string() });
        variables.set("TARGET", target.clone());
        variables.set("HOST", target.clone());
        variables.set("ARCH", target.split('_').next().unwrap().to_string());

        let mut template_dir = cookbook_dir.as_ref().to_path_buf();
        template_dir.push("templates");

        let mut recipe_dir = cookbook_dir.as_ref().to_path_buf();
        recipe_dir.push("recipes");
        recipe_dir.push(package);
        std::env::set_current_dir(recipe_dir)?;

        for entry in fs::read_dir(&template_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() &&
               entry.path().extension() == Some(OsStr::new("ion")) {
                shell.execute_command(File::open(entry.path())?)?;
            }
        }

        shell.execute_command(File::open("recipe.ion")?)?;

        Ok(Recipe { target, shell, debug, cookbook_dir: cookbook_dir.as_ref().to_path_buf() })
    }

    fn src(&self) -> Result<Source> {
        // Syntax based on Arch PKGBUILD
        // TODO: Change to associative array when supported
        let src = self.shell.variables().get_str("src")
            .or(Err(CookError::MissingVar("src".to_string())))?;

        if src.starts_with("git://") {
            let mut parts = src.splitn(2, "#branch=");
            let url = parts.next().unwrap().to_string();
            let branch = parts.next().map(str::to_string);
            Ok(Source::Git(url, branch))
        } else if src.starts_with("git+") {
            let mut parts = src[4..].splitn(2, "#branch=");
            let url = parts.next().unwrap().to_string();
            let branch = parts.next().map(str::to_string);
            Ok(Source::Git(url, branch))
        } else {
            Ok(Source::Tar(src.to_string()))
        }
    }

    fn call_func_src(&mut self, func: &str, args: &[&str]) -> Result<()> {
        call_func_src(&mut self.shell, func, args)
    }

    /// Return the metadata, from which /pkg/<package>.toml is generated.
    /// This calls the recipe's version(), so it will fail if that does.
    pub fn meta(&mut self) -> Result<PackageMeta> {
        let version = self.version()?;
        let variables = self.shell.variables();
        let name = variables.get_str("name")
            .or(Err(CookError::MissingVar("name".to_string())))?;
        // TODO best way to handle incorrect type?
        let depends = if let Some(Value::Array(arr)) = variables.get("depends") {
            arr.iter().map(|d| {
                if let Value::Str(s) = d {
                    s.to_string()
                } else {
                    todo!();
                }
            }).collect()
        } else {
            Vec::new()
        };

        Ok(PackageMeta {
            name: name.to_string(),
            version: version.to_string(),
            target: self.target.clone(),
            depends
        })
    }

    fn build_depends(&'a self) -> impl Iterator<Item = &'a str> {
        // TODO best way to handle incorrect type?
        let variables = self.shell.variables();
        let deps = if let Some(Value::Array(arr)) = variables.get("build_depends") {
            Some(arr.iter())
        } else {
            None
        };
        deps.into_iter().flatten().map(|d| {
            if let Value::Str(s) = d {
                s.deref()
            } else {
                todo!();
            }
        })
    }

    pub fn tar(&mut self) -> Result<()> {
        let meta = self.meta()?;
        fs::create_dir_all("stage/pkg")?;
        let mut manifest = File::create(format!("stage/pkg/{}.toml", meta.name))?;
        manifest.write_all(meta.to_toml().as_bytes())?;
        drop(manifest);

        let repo = Repo::new(&self.target);
        repo.create("stage")?;
        Ok(())
    }

    pub fn untar(&self) -> Result<()> {
        try_ifexist!(fs::remove_file("stage.tar"));
        Ok(())
    }

    pub fn fetch(&self) -> Result<()> {
        match self.src()? {
            Source::Git(url, branch) => {
                if !Path::new("source").is_dir() {
                    let mut command = Command::new("git");
                    command.args(&["clone", "--recursive", &url, "source"]);

                    if let Some(branch) = branch {
                        command.args(&["--branch", &branch]);
                    }
                    
                    try_process_status!("git", command.status()?);
                } else {
                    macro_rules! git_cmd {
                        ( $( $arg:expr ),+ ) => {
                            {
                                let status = Command::new("git")
                                    .args(&["-C", "source", $( $arg ),+]).status()?;
                                try_process_status!("git", status);

                            }
                        }
                    }

                    git_cmd!("remote", "set-url", "origin", &url);
                    git_cmd!("fetch", "origin");
                    git_cmd!("submodule", "sync", "--recursive");
                    git_cmd!("submodule", "update", "--init", "--recursive");
                }
            },
            Source::Tar(url) => {
                if !Path::new("source.tar").is_file() {
                    download(&url, "source.tar")?;
                }

                if !Path::new("source").is_dir() {
                    // It might be nice to use the tar crate, but that doesn't
                    // handle compression. The logic for detecting and handling
                    // compression is in Redox's tar command though, and
                    // could possibly be shared.
                    fs::create_dir("source")?;
                    let status = Command::new("tar")
                        .args(&["xvf", "source.tar", "-C", "source",
                                "--strip-components", "1"]).status()?;

                    try_process_status!("tar", status);
                }
            }
        }
        Ok(())
    }

    pub fn unfetch(&self) -> Result<()> {
        try_ifexist!(fs::remove_dir_all("source"));
        try_ifexist!(fs::remove_file("source.tar"));
        Ok(())
    }

    pub fn prepare(&self) -> Result<()> {
        self.unprepare()?;

        for depend in self.build_depends() {
            // XXX have some way to rebuild iff no built debug; have two copies
            let mut recipe = Recipe::new(self.target.clone(), self.cookbook_dir.clone(), depend, self.debug)?;
            recipe.fetch()?;
            recipe.build()?; 
            // XXX behave like repo.sh
        }

        Ok(())
    }

    pub fn unprepare(&self) -> Result<()> {
        try_ifexist!(fs::remove_dir_all("build"));
        Ok(())
    }

    pub fn build(&mut self) -> Result<()> {
        self.call_func_src("build", &[])
    }

    pub fn test(&mut self) -> Result<()> {
        self.call_func_src("test", &[])
    }

    pub fn clean(&mut self) -> Result<()> {
        self.call_func_src("clean", &[])
    }

    pub fn stage(&mut self) -> Result<()> {
        self.unstage()?;
        fs::create_dir("stage")?;
        let path = fs::canonicalize("./stage")?;
        self.call_func_src("stage", &[path.to_str().unwrap()])
    }

    pub fn unstage(&self) -> Result<()> {
        try_ifexist!(fs::remove_dir_all("stage"));
        Ok(())
    }

    pub fn version(&mut self) -> Result<String> {
        let mut ver = String::new();
        // TODO FIX
        /*
        let res = self.shell.fork(Capture::Stdout, |shell| {
            call_func_src(shell, "version", &[]).unwrap();
        })?;
        res.stdout.unwrap().read_to_string(&mut ver)?;
        // XXX non-zero return
        if ver.ends_with("\n") {
            ver.pop();
        }
        */
        Ok(ver)
    }
}
