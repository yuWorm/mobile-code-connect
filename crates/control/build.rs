use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-env-changed=MOBILECODE_CONNECT_WEB_DIST");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let dist_dir = env::var_os("MOBILECODE_CONNECT_WEB_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../../web/dist"));
    let out_path = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("embedded_web.rs");

    println!("cargo:rerun-if-changed={}", dist_dir.display());
    if !dist_dir.is_dir() {
        write_unavailable(&out_path)?;
        return Ok(());
    }

    let mut files = Vec::new();
    collect_files(&dist_dir, &mut files)?;
    files.sort();

    if files.is_empty() {
        write_unavailable(&out_path)?;
        return Ok(());
    }

    let mut output = String::new();
    output.push_str(
        "#[derive(Clone, Copy)]\n\
         pub(crate) struct EmbeddedWebAsset {\n\
             pub(crate) bytes: &'static [u8],\n\
             pub(crate) content_type: &'static str,\n\
         }\n\n\
         pub(crate) fn embedded_web_available() -> bool {\n\
             true\n\
         }\n\n\
         pub(crate) fn embedded_web_asset(path: &str) -> Option<EmbeddedWebAsset> {\n\
             let path = path.trim_start_matches('/');\n\
             match path {\n",
    );

    for path in files {
        println!("cargo:rerun-if-changed={}", path.display());
        let relative_path = path
            .strip_prefix(&dist_dir)
            .expect("asset lives under web dist")
            .to_string_lossy()
            .replace('\\', "/");
        let include_path = path.canonicalize()?.to_string_lossy().into_owned();
        let content_type = content_type_for(&relative_path);

        output.push_str("                ");
        if relative_path == "index.html" {
            output.push_str("\"\" | ");
        }
        output.push_str(&format!(
            "{} => Some(EmbeddedWebAsset {{ bytes: include_bytes!({}), content_type: {} }}),\n",
            raw_string_literal(&relative_path),
            raw_string_literal(&include_path),
            raw_string_literal(content_type),
        ));
    }

    output.push_str(
        "                _ => None,\n\
             }\n\
         }\n",
    );

    let mut file = fs::File::create(out_path)?;
    file.write_all(output.as_bytes())
}

fn write_unavailable(out_path: &Path) -> io::Result<()> {
    fs::write(
        out_path,
        "#[derive(Clone, Copy)]\n\
         pub(crate) struct EmbeddedWebAsset {\n\
             pub(crate) bytes: &'static [u8],\n\
             pub(crate) content_type: &'static str,\n\
         }\n\n\
         pub(crate) fn embedded_web_available() -> bool {\n\
             false\n\
         }\n\n\
         pub(crate) fn embedded_web_asset(_path: &str) -> Option<EmbeddedWebAsset> {\n\
             None\n\
         }\n",
    )
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn content_type_for(path: &str) -> &'static str {
    match Path::new(path).extension().and_then(|ext| ext.to_str()) {
        Some("css") => "text/css; charset=utf-8",
        Some("gif") => "image/gif",
        Some("html") => "text/html; charset=utf-8",
        Some("ico") => "image/x-icon",
        Some("jpeg" | "jpg") => "image/jpeg",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("map") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("txt") => "text/plain; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

fn raw_string_literal(value: &str) -> String {
    let mut hashes = String::new();
    loop {
        let terminator = format!("\"{}", hashes);
        if !value.contains(&terminator) {
            return format!("r{}\"{}\"{}", hashes, value, hashes);
        }
        hashes.push('#');
    }
}
