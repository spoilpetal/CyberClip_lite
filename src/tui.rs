
use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};
use ratatui_image::{
    picker::Picker,
    protocol::StatefulProtocol,
    StatefulImage,
};
use std::{
    fs,
    io,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

#[derive(serde::Deserialize, serde::Serialize, Clone)]
struct Entry {
    kind: String,
    value: String,
    #[serde(default)]
    pinned: bool,
}

// Keep history entries visually consistent with the file browser.
fn history_icon(entry: &Entry) -> &'static str {
    match entry.kind.as_str() {
        "text" => "📝",
        "image" => "🖼️",
        "files" => "📁",
        "file" => {
            let path = Path::new(&entry.value);
            match path.extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("")
                .to_lowercase()
                .as_str()
            {
                "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "svg" => "🖼️",
                "rs" | "py" | "js" | "ts" | "lua" | "c" | "cpp" | "html" | "css" => "💻",
                "json" | "toml" | "yaml" | "yml" => "⚙️",
                "txt" | "md" | "log" => "📄",
                "pdf" => "📕",
                "zip" | "tar" | "gz" | "7z" | "rar" => "📦",
                "mp3" | "wav" | "ogg" | "flac" => "🎵",
                "mp4" | "mkv" | "webm" | "mov" | "avi" => "🎬",
                _ => "📄",
            }
        }
        _ => "📋",
    }
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
    fs::write(
        history_path()?,
        serde_json::to_string_pretty(entries)?,
    )?;
    Ok(())
}

fn restore(entry: &Entry) -> Result<()> {
    let mut command = Command::new("wl-copy");

    let data: Vec<u8> = match entry.kind.as_str() {
        "text" => entry.value.as_bytes().to_vec(),

        "image" => {
            let path = Path::new(&entry.value);
            let extension = path.extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();

            let mime = match extension.as_str() {
                "png" => "image/png",
                "jpg" | "jpeg" => "image/jpeg",
                "webp" => "image/webp",
                "gif" => "image/gif",
                "bmp" => "image/bmp",
                "svg" => "image/svg+xml",
                _ => anyhow::bail!("Unknown image format"),
            };

            command.arg("--type").arg(mime);
            fs::read(path)?
        }

        "file" => {
            command.arg("--type").arg("text/uri-list");
            format!("file://{}\r\n", entry.value).into_bytes()
        }

        _ => anyhow::bail!("Unknown history item"),
    };

    let mut child = command
        .stdin(Stdio::piped())
        .spawn()
        .context("Could not start wl-copy")?;

    child.stdin.take()
        .context("Clipboard input unavailable")?
        .write_all(&data)?;

    if !child.wait()?.success() {
        anyhow::bail!("Clipboard operation failed");
    }

    Ok(())
}

pub fn run() -> Result<()> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Detect Kitty graphics support after entering alternate screen.
    let mut picker = Picker::from_query_stdio()
        .unwrap_or_else(|_| Picker::halfblocks());

    let result = app(&mut terminal, &mut picker);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    picker: &mut Picker,
) -> Result<()> {
    let mut selected = 0usize;
    let mut search = String::new();
    let mut searching = false;
    let mut message = String::from("CyberClip ready");

    let mut preview_scroll = 0u16;
    let mut previous_selection = String::new();

    
    let mut image_protocol: Option<StatefulProtocol> = None;
    let mut preview_error: Option<String> = None;

    loop {
        let mut entries = load_history()?;

        if !search.is_empty() {
            let query = search.to_lowercase();
            entries.retain(|e| e.value.to_lowercase().contains(&query));
        }

        if !entries.is_empty() {
            selected = selected.min(entries.len() - 1);
        } else {
            selected = 0;
        }

        let current = entries.get(selected).cloned();

        // Refresh preview only when selection changes.
        let identity = current.as_ref()
            .map(|e| format!("{}:{}", e.kind, e.value))
            .unwrap_or_default();

        if identity != previous_selection {
            preview_scroll = 0;
            previous_selection = identity;

            image_protocol = None;
            
            preview_error = None;

            if let Some(entry) = &current {
                if entry.kind == "image" {
                    let path = Path::new(&entry.value);

                    match image::ImageReader::open(path)
                        .context("Cannot open image")
                        .and_then(|reader| {
                            reader.decode().context("Cannot decode image")
                        })
                    {
                        Ok(image) => {
                            image_protocol =
                                Some(picker.new_resize_protocol(image));
                            
                        }
                        Err(error) => {
                            preview_error = Some(error.to_string());
                        }
                    }
                }
            }
        }

        terminal.draw(|frame| {
            let area = frame.area();

            let main = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(8),
                    Constraint::Length(3),
                ])
                .split(area);

            let header = Paragraph::new(
                "  CYBERCLIP // CLIPBOARD CONTROL"
            )
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .block(Block::default().borders(Borders::ALL));

            frame.render_widget(header, main[0]);

            let panels = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(42),
                    Constraint::Percentage(58),
                ])
                .split(main[1]);

            // LEFT: HISTORY
            let items: Vec<ListItem> = entries
                .iter()
                .enumerate()
                .map(|(i, entry)| {
                    let pin = if entry.pinned { " [PIN]" } else { "" };

                    let preview: String = entry.value
                        .lines()
                        .next()
                        .unwrap_or("")
                        .chars()
                        .take(35)
                        .collect();

                    let icon = history_icon(entry);

                    ListItem::new(format!(
                        "{}. {} [{}] {}{}",
                        i + 1,
                        icon,
                        entry.kind.to_uppercase(),
                        preview,
                        pin
                    ))
                })
                .collect();

            let mut state = ListState::default();

            if !entries.is_empty() {
                state.select(Some(selected));
            }

            let list = List::new(items)
                .block(
                    Block::default()
                        .title(" HISTORY ")
                        .borders(Borders::ALL),
                )
                .highlight_style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("▶ ");

            frame.render_stateful_widget(list, panels[0], &mut state);

            // RIGHT: PREVIEW
            let preview_block = Block::default()
                .title(" LIVE PREVIEW ")
                .borders(Borders::ALL);

            let preview_area = preview_block.inner(panels[1]);
            frame.render_widget(preview_block, panels[1]);

            if let Some(entry) = &current {
                match entry.kind.as_str() {
                    "text" => {
                        let text = Paragraph::new(entry.value.as_str())
                            .wrap(Wrap { trim: false })
                            .scroll((preview_scroll, 0));

                        frame.render_widget(text, preview_area);
                    }

                    "image" => {
                        if let Some(error) = &preview_error {
                            let text = Paragraph::new(format!(
                                "Image preview unavailable:\n\n{}",
                                error
                            ));

                            frame.render_widget(text, preview_area);
                        } else if let Some(protocol) = image_protocol.as_mut() {
                            frame.render_stateful_widget(
                                StatefulImage::default(),
                                preview_area,
                                protocol,
                            );
                        } else {
                            frame.render_widget(
                                Paragraph::new("Loading image preview..."),
                                preview_area,
                            );
                        }
                    }

                    "file" => {
                        let path = Path::new(&entry.value);

                        let details = match fs::metadata(path) {
                            Ok(metadata) => format!(
                                "FILE DETAILS\n\nName: {}\n\nPath: {}\n\nSize: {} bytes\n\nType: File reference",
                                path.file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy(),
                                entry.value,
                                metadata.len()
                            ),
                            Err(_) => format!(
                                "FILE DETAILS\n\nPath: {}\n\nFile is no longer available.",
                                entry.value
                            ),
                        };

                        frame.render_widget(
                            Paragraph::new(details)
                                .wrap(Wrap { trim: false }),
                            preview_area,
                        );
                    }

                    _ => {
                        frame.render_widget(
                            Paragraph::new("Unknown clipboard item"),
                            preview_area,
                        );
                    }
                }
            } else {
                frame.render_widget(
                    Paragraph::new("No clipboard items"),
                    preview_area,
                );
            }

            // FOOTER
            let footer = Paragraph::new(format!(
                "{} | Search: {} | ↑↓ Navigate | Enter Restore | P Pin | Del Remove | / Search | PgUp/PgDn Scroll | Q Quit",
                message, search
            ))
            .style(Style::default().fg(Color::Green))
            .block(Block::default().borders(Borders::ALL));

            frame.render_widget(footer, main[2]);
        })?;

        if !event::poll(Duration::from_millis(200))? {
            continue;
        }

        let Event::Key(key) = event::read()? else {
            continue;
        };

        if key.kind != KeyEventKind::Press {
            continue;
        }

        if searching {
            match key.code {
                KeyCode::Esc => {
                    searching = false;
                    search.clear();
                }
                KeyCode::Enter => searching = false,
                KeyCode::Backspace => {
                    search.pop();
                    selected = 0;
                }
                KeyCode::Char(c) => {
                    search.push(c);
                    selected = 0;
                }
                _ => {}
            }
            continue;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => break,

            KeyCode::Down => {
                if selected + 1 < entries.len() {
                    selected += 1;
                }
            }

            KeyCode::Up => {
                selected = selected.saturating_sub(1);
            }

            KeyCode::PageDown => {
                preview_scroll = preview_scroll.saturating_add(5);
            }

            KeyCode::PageUp => {
                preview_scroll = preview_scroll.saturating_sub(5);
            }

            KeyCode::Char('/') => {
                searching = true;
                search.clear();
                selected = 0;
            }

            KeyCode::Enter => {
                if let Some(entry) = entries.get(selected) {
                    match restore(entry) {
                        Ok(_) => message = "Clipboard restored!".into(),
                        Err(e) => message = format!("Error: {}", e),
                    }
                }
            }

            KeyCode::Char('p') | KeyCode::Char('P') => {
                if let Some(entry) = entries.get(selected) {
                    let mut all = load_history()?;

                    if let Some(original) = all.iter_mut().find(|e| {
                        e.kind == entry.kind && e.value == entry.value
                    }) {
                        original.pinned = !original.pinned;
                        save_history(&all)?;
                        message = "Pin status changed".into();
                    }
                }
            }

            KeyCode::Delete => {
                if let Some(entry) = entries.get(selected) {
                    let mut all = load_history()?;

                    if let Some(pos) = all.iter().position(|e| {
                        e.kind == entry.kind && e.value == entry.value
                    }) {
                        all.remove(pos);
                        save_history(&all)?;
                        message = "History item removed".into();
                        selected = selected.saturating_sub(1);
                    }
                }
            }

            _ => {}
        }
    }

    Ok(())
}
