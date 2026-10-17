use std::ffi::OsStr;
use std::path::Path;

use orrin_core::Result;

use super::Checkout;
use super::command::run;

const SETTINGS: [&str; 5] = ["--no-pager", "-c", "advice.detachedHead=false", "-c", "core.autocrlf=false"];
const SHALLOW: [&str; 6] = ["clone", "--quiet", "--depth", "1", "--recurse-submodules", "--shallow-submodules"];

pub async fn fetch(url: &str, checkout: &Checkout, into: &Path) -> Result<()> {
    let (url, into) = (OsStr::new(url), into.as_os_str());
    match checkout {
        Checkout::Tag(tag) => {
            let mut args: Vec<&OsStr> = SHALLOW.iter().map(OsStr::new).collect();
            args.extend([OsStr::new("--branch"), OsStr::new(tag), url, into]);
            git(&args, None).await
        }
        Checkout::Rev(rev) => {
            git(&["clone".as_ref(), "--quiet".as_ref(), "--no-checkout".as_ref(), url, into], None).await?;
            let at = Some(Path::new(into));
            git(&["checkout".as_ref(), "--quiet".as_ref(), rev.as_ref()], at).await?;
            git(&["submodule", "update", "--quiet", "--init", "--recursive"].map(OsStr::new), at).await
        }
    }
}

async fn git(args: &[&OsStr], dir: Option<&Path>) -> Result<()> {
    let all: Vec<&OsStr> = SETTINGS.iter().map(OsStr::new).chain(args.iter().copied()).collect();
    run("git", &all, dir).await
}
