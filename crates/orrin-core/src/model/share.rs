use crate::model::{Entry, Mode};

pub const SHARE_FROM: u64 = 256 * 1024;

const TEXT: &str = "bat c cc cfg cjs cmd conf cpp cs css csv cxx dart editorconfig env gd gdshader gitattributes \
    gitignore glsl go godot gradle h hlsl hpp htm html ini java js json jsonc jsx kt less lock log lua md mdx mjs mts \
    php props ps1 py rb rs sass scss sh sql svelte svg swift targets tex toml tres tscn ts tsx txt vcxproj vue wgsl \
    xml yaml yml zig";

impl Entry {
    pub fn shareable(&self) -> bool {
        self.mode == Mode::File && self.size >= SHARE_FROM && !is_text_path(&self.path)
    }
}

pub fn is_text_path(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    match name.rsplit_once('.') {
        Some((_, ext)) => TEXT.split_whitespace().any(|known| known == ext),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::Hash;

    fn entry(path: &str, size: u64) -> Entry {
        Entry { path: path.into(), hash: Hash::of(b""), size, mode: Mode::File }
    }

    #[test]
    fn shares_only_big_binary_files() {
        assert!(entry("art/hero.png", SHARE_FROM).shareable());
        assert!(!entry("art/hero.png", 1000).shareable());
        assert!(!entry("src/huge.rs", 10 * SHARE_FROM).shareable());
        assert!(!entry("Makefile", 10 * SHARE_FROM).shareable());
    }
}
