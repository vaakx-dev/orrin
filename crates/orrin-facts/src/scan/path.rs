use std::path::PathBuf;

pub fn which(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    let extensions = extensions(name);
    std::env::split_paths(&paths)
        .flat_map(|dir| extensions.iter().map(move |ext| dir.join(format!("{name}{ext}"))))
        .find(|candidate| candidate.is_file())
}

fn extensions(name: &str) -> Vec<String> {
    if !cfg!(windows) || name.contains('.') {
        return vec![String::new()];
    }
    let listed = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
    listed.split(';').filter(|e| !e.is_empty()).map(|e| e.to_lowercase()).collect()
}
