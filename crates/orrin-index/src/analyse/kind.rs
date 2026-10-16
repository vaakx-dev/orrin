use crate::documents::Format;
use crate::symbols::Lang;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Code(Lang),
    Markdown,
    Scene,
    Document(Format),
    Plain,
    Skip,
}

const SKIPPED: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "ico", "tga", "tif", "tiff", "psd", "exr", "hdr", "ktx", "dds", "glb",
    "gltf", "fbx", "obj", "blend", "blend1", "wav", "ogg", "mp3", "flac", "mp4", "mov", "avi", "webm", "mkv", "ttf",
    "otf", "woff", "woff2", "zip", "7z", "gz", "tar", "rar", "exe", "dll", "so", "dylib", "lib", "a", "o", "pdb",
    "bin", "pck", "res", "scn", "ctex", "stex", "import", "sqlite", "db",
];

impl Kind {
    pub fn of(path: &str) -> Self {
        let name = path.rsplit('/').next().unwrap_or(path);
        let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
        if let Some(lang) = Lang::from_extension(&ext) {
            return Kind::Code(lang);
        }
        if let Some(format) = Format::from_extension(&ext) {
            return Kind::Document(format);
        }
        match ext.as_str() {
            "md" | "markdown" | "mdx" => Kind::Markdown,
            "tscn" | "escn" => Kind::Scene,
            e if SKIPPED.contains(&e) => Kind::Skip,
            _ => Kind::Plain,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Kind::Code(lang) => lang.key(),
            Kind::Markdown => "md",
            Kind::Scene => "tscn",
            Kind::Document(format) => format.key(),
            Kind::Plain => "plain",
            Kind::Skip => "skip",
        }
    }
}
