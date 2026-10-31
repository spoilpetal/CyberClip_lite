mod tui;
mod files;
mod get;
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Parser)]
#[command(name = "cyberclip", version, about = "CyberArch clipboard manager")]
struct Cli {
    #[command(subcommand)]
    command: Action,
}

#[derive(Subcommand)]
enum Action {
    Copy { #[arg(required = true, num_args = 1..)] paths: Vec<PathBuf> },
    Text { content: String },
    Get {
       #[arg(required = true, num_args = 1..=3)]
       args: Vec<String>,
       #[arg(short = 'n')]
       no_numbers: bool,
    },
    History,
    Search { query: String },
    Paste { index: Option<usize> },
    Remove { index: usize },
    Pin { index: usize },
    Clear,
    Ui,
    Files,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    kind: String,
    value: String,
    #[serde(default)]
    pinned: bool,
}

fn history_path() -> Result<PathBuf> {
    let dir = dirs::data_local_dir()
        .context("Cannot find local data directory")?
        .join("cyberclip");

    fs::create_dir_all(&dir)?;
    Ok(dir.join("history.json"))
}

fn load_history() -> Result<Vec<Entry>> {
    let path = history_path()?;

    if !path.exists() {
        return Ok(Vec::new());
    }

    let data = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&data).unwrap_or_default())
}

fn save_history(entries: &[Entry]) -> Result<()> {
    let path = history_path()?;
    fs::write(path, serde_json::to_string_pretty(entries)?)?;
    Ok(())
}

pub(crate) fn clipboard(data: &[u8], mime: Option<&str>) -> Result<()> {
    let mut cmd = Command::new("wl-copy");

    if let Some(mime) = mime {
        cmd.arg("--type").arg(mime);
    }

    let mut child = cmd
        .stdin(Stdio::piped())
        .spawn()
        .context("Could not start wl-copy")?;

    child
        .stdin
        .take()
        .context("Clipboard input unavailable")?
        .write_all(data)?;

    let status = child.wait()?;

    if !status.success() {
        bail!("Clipboard operation failed");
    }

    Ok(())
}

fn add_history(kind: &str, value: String) -> Result<()> {
    let mut entries = load_history()?;

    entries.insert(
        0,
        Entry {
            kind: kind.to_string(),
            value,
            pinned: false,
        },
    );

    // Keep every pinned item and the 100 newest unpinned items.
    let mut unpinned_count = 0;

    entries.retain(|entry| {
        if entry.pinned {
            true
        } else {
            unpinned_count += 1;
            unpinned_count <= 100
        }
    });

    save_history(&entries)
}
fn mime_type(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        "bmp" => Some("image/bmp"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

fn file_uri(path: &Path) -> Result<String> {
    let path = fs::canonicalize(path)
        .with_context(|| format!("File not found: {}", path.display()))?;
    if !path.is_file() {
        bail!("Please provide files, not directories");
    }
    #[cfg(unix)]
    let raw = {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().to_vec()
    };
    #[cfg(not(unix))]
    let raw = path.to_string_lossy().as_bytes().to_vec();

    let mut encoded = String::from("file://");
    for byte in raw {
        if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{:02X}", byte));
        }
    }
    Ok(encoded)
}

fn copy_files(paths: Vec<PathBuf>) -> Result<()> {
    if paths.is_empty() {
        bail!("Provide at least one file");
    }
    let mut canonical = Vec::with_capacity(paths.len());
    for path in paths {
        let resolved = fs::canonicalize(&path)
            .with_context(|| format!("File not found: {}", path.display()))?;
        if !resolved.is_file() {
            bail!("Please provide files, not directories: {}", resolved.display());
        }
        canonical.push(resolved);
    }

    // A single image remains available as native image data.
    if canonical.len() == 1 {
        let path = &canonical[0];
        if let Some(mime) = mime_type(path) {
            clipboard(&fs::read(path)?, Some(mime))?;
            add_history("image", path.to_string_lossy().into_owned())?;
            println!("CyberClip: image copied successfully.");
            return Ok(());
        }
    }

    let mut uris = String::new();
    for path in &canonical {
        uris.push_str(&file_uri(path)?);
        uris.push_str("\r\n");
    }
    clipboard(uris.as_bytes(), Some("text/uri-list"))?;
    let history = canonical.iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>().join("\n");
    add_history(if canonical.len() == 1 { "file" } else { "files" }, history)?;
    println!("CyberClip: {} file reference(s) copied.", canonical.len());
    Ok(())
}

fn restore(entry: &Entry) -> Result<()> {
    match entry.kind.as_str() {
        "text" => clipboard(entry.value.as_bytes(), None)?,
        "image" => {
            let path = Path::new(&entry.value);
            let mime = mime_type(path).context("Unknown image format")?;
            clipboard(&fs::read(path)?, Some(mime))?;
        }
        "file" => {
            let uri = format!("{}\r\n", file_uri(Path::new(&entry.value))?);
            clipboard(uri.as_bytes(), Some("text/uri-list"))?;
        }
        "files" => {
            let mut uris = String::new();
            for path in entry.value.lines() {
                uris.push_str(&file_uri(Path::new(path))?);
                uris.push_str("\r\n");
            }
            clipboard(uris.as_bytes(), Some("text/uri-list"))?;
        }
        _ => bail!("Unknown history item type"),
    }

    println!("CyberClip: clipboard restored.");
    Ok(())
}

fn get_index(index: usize, entries: &[Entry]) -> Result<usize> {
    if index == 0 || index > entries.len() {
        bail!("Invalid history number. Use cyberclip history to see available items.");
    }

    Ok(index - 1)
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Action::Copy { paths } => copy_files(paths)?,
        Action::Files => files::run()?,
        Action::Ui => tui::run()?,
        Action::Get { args, no_numbers } => {
            let show_numbers = !no_numbers;
            let result = match args.as_slice() {
                [file] => get::whole_file(Path::new(file))?,
                [query, file] => get::search(Path::new(file), query, show_numbers)?,
                [mode, range, file] if mode == "line" => {
                    let (start, end) = if let Some((start, end)) = range.split_once(':') {
                        (start.parse::<usize>()?, end.parse::<usize>()?)
                    } else {
                        let number = range.parse::<usize>()?;
                        (number, number)
                    };
                    get::extract(Path::new(file), start, end, show_numbers)?
                }
                _ => bail!("Usage: cyberclip get FILE | get QUERY FILE | get line RANGE FILE"),
            };

            if result.matches == 0 || result.text.is_empty() {
                println!("No content found. Clipboard unchanged.");
            } else {
                clipboard(result.text.as_bytes(), None)?;
                add_history("text", result.text.clone())?;
                print!("{}", result.text);
                println!("\n{} line(s) copied to clipboard.", result.matches);
            }
        }
        Action::Text { content } => {
            clipboard(content.as_bytes(), None)?;
            add_history("text", content)?;
            println!("CyberClip: text copied.");
        }

        Action::History => {
            let entries = load_history()?;

            if entries.is_empty() {
                println!("Clipboard history is empty.");
            }

            for (i, entry) in entries.iter().enumerate() {
                let preview: String = entry.value.chars().take(65).collect();
                let pin = if entry.pinned { " [PINNED]" } else { "" };

                println!("{}: [{}] {}{}", i + 1, entry.kind, preview, pin);
            }
        }

        Action::Search { query } => {
            let entries = load_history()?;

            for (i, entry) in entries.iter().enumerate() {
                if entry.value.to_lowercase().contains(&query.to_lowercase()) {
                    println!("{}: [{}] {}", i + 1, entry.kind, entry.value);
                }
            }
        }

        Action::Paste { index } => {
            let entries = load_history()?;
            let number = index.unwrap_or(1);
            let position = get_index(number, &entries)?;
            restore(&entries[position])?;
        }

        Action::Remove { index } => {
            let mut entries = load_history()?;
            let position = get_index(index, &entries)?;

            entries.remove(position);
            save_history(&entries)?;

            println!("CyberClip: item {} removed.", index);
        }

        Action::Pin { index } => {
            let mut entries = load_history()?;
            let position = get_index(index, &entries)?;

            entries[position].pinned = !entries[position].pinned;
            let pinned = entries[position].pinned;

            save_history(&entries)?;

            if pinned {
                println!("CyberClip: item {} pinned.", index);
            } else {
                println!("CyberClip: item {} unpinned.", index);
            }
        }

        Action::Clear => {
            clipboard(b"", None)?;
            save_history(&[])?;
            println!("CyberClip: clipboard history cleared.");
        }
    }

    Ok(())
}
