
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
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};
use ratatui_image::{
    picker::Picker,
    protocol::StatefulProtocol,
    StatefulImage,
};
use std::{
    collections::VecDeque,
    fs,
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::Duration,
};
use syntect::{
    easy::HighlightLines,
    highlighting::ThemeSet,
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

struct FileEntry {
    path: PathBuf,
    name: String,
    is_dir: bool,
    size: u64,
}

enum Preview {
    Text(Vec<Line<'static>>),
    Archive(Vec<Line<'static>>),
    LargeText {
       pages: VecDeque<TextPage>,
       current: usize,
    },
    Image(StatefulProtocol, String),
    Info(String),
}



const CHUNK_SIZE: usize = 2 * 1024 * 1024;
const MAX_LINE_SIZE: usize = 1024 * 1024;

struct TextPage {
    lines: Vec<String>,
    highlighted: Vec<Line<'static>>,
    first_line: u64,
    next_offset: u64,
    eof: bool,
}

fn read_text_page(
    path: &Path,
    offset: u64,
    first_line: u64,
) -> Result<TextPage> {
    let mut file = fs::File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;

    let mut bytes = vec![0u8; CHUNK_SIZE];
    let count = file.read(&mut bytes)?;
    bytes.truncate(count);

    let mut eof = count == 0;

    // Finish the final partial line unless the file has ended.
    if count == CHUNK_SIZE {
        let mut extra = [0u8; 1];

        while !bytes.ends_with(b"\n") {
            if file.read(&mut extra)? == 0 {
                eof = true;
                break;
            }

            bytes.push(extra[0]);

            if bytes.len() > CHUNK_SIZE + MAX_LINE_SIZE {
                anyhow::bail!("Line exceeds the 1 MB safety limit");
            }

            if extra[0] == b'\n' {
                break;
            }
        }
    } else {
        eof = true;
    }

    let next_offset = offset + bytes.len() as u64;

    let content = String::from_utf8_lossy(&bytes);

    let lines = content
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();

    Ok(TextPage {
        lines,
        highlighted: Vec::new(),
        first_line,
        next_offset,
        eof,
    })
}


fn highlight_page(
    page: &mut TextPage,
    path: &Path,
    syntax_set: &SyntaxSet,
    theme_set: &ThemeSet,
) {
    if !page.highlighted.is_empty() {
        return;
    }

    let syntax = syntax_set
        .find_syntax_for_file(path)
        .ok()
        .flatten()
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

    let Some(theme) = theme_set.themes.get("base16-ocean.dark") else {
        return;
    };

    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut result = Vec::new();

    for (index, content) in page.lines.iter().enumerate() {
        let mut spans = vec![Span::styled(
            format!("{:>6} ", page.first_line + index as u64),
            Style::default().fg(Color::DarkGray),
        )];

        if let Ok(ranges) = highlighter.highlight_line(content, syntax_set) {
            for (style, text) in ranges {
                spans.push(Span::styled(
                    text.to_string(),
                    Style::default().fg(Color::Rgb(
                        style.foreground.r,
                        style.foreground.g,
                        style.foreground.b,
                    )),
                ));
            }
        }

        result.push(Line::from(spans));
    }

    page.highlighted = result;
}

fn read_directory(path: &Path) -> Result<Vec<FileEntry>> {
    let mut entries = Vec::new();

    for item in fs::read_dir(path)? {
        let item = item?;
        let item_path = item.path();
        let metadata = fs::symlink_metadata(&item_path)?;

        entries.push(FileEntry {
            name: item.file_name().to_string_lossy().into_owned(),
            path: item_path,
            is_dir: metadata.is_dir(),
            size: metadata.len(),
        });
    }

    entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(entries)
}


fn file_icon(entry: &FileEntry) -> &'static str {
    if entry.is_dir {
        return "📁";
    }

    match entry.path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => "🖼 ",
        "rs" | "py" | "js" | "ts" | "lua" | "c" | "cpp" | "html" | "css" => "💻",
        "json" | "toml" | "yaml" | "yml" => "⚙ ",
        "txt" | "md" | "log" => "📄",
        "pdf" => "📕",
        "zip" | "tar" | "gz" | "7z" => "📦",
        "mp3" | "wav" | "ogg" => "🎵",
        "mp4" | "mkv" | "webm" => "🎬",
        _ => "📄",
    }
}

fn is_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp"
    )
}

const MAX_ARCHIVE_ENTRIES_TO_DISPLAY: usize = 500;

fn make_zip_preview(path: &Path, archive_size: u64) -> Preview {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) => return Preview::Info(format!("Cannot open ZIP archive: {error}")),
    };

    let mut archive = match zip::ZipArchive::new(file) {
        Ok(archive) => archive,
        Err(error) => return Preview::Info(format!("Invalid or unsupported ZIP archive:\n{error}")),
    };

    let total_entries = archive.len();
    let mut lines = vec![
        Line::from(Span::styled(
            format!("ZIP ARCHIVE | {total_entries} entries | {archive_size} bytes"),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("{:<5} {:<12} {:>12}  {}", "TYPE", "COMPRESSED", "SIZE", "NAME"),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
    ];

    let display_count = total_entries.min(MAX_ARCHIVE_ENTRIES_TO_DISPLAY);
    let mut unpacked_total = 0u64;
    let mut compressed_total = 0u64;

    for index in 0..display_count {
        let entry = match archive.by_index(index) {
            Ok(entry) => entry,
            Err(error) => {
                lines.push(Line::from(Span::styled(
                    format!("Could not read entry #{index}: {error}"),
                    Style::default().fg(Color::Red),
                )));
                continue;
            }
        };

        let is_dir = entry.is_dir();
        let name = entry.name().to_string();
        let size = entry.size();
        let compressed = entry.compressed_size();
        unpacked_total = unpacked_total.saturating_add(size);
        compressed_total = compressed_total.saturating_add(compressed);
        let kind = if is_dir { "DIR" } else { "FILE" };
        let display_name = if is_dir { format!("📁 {name}") } else { format!("📄 {name}") };

        lines.push(Line::from(vec![
            Span::styled(
                format!("{kind:<5} {compressed:>12} {size:>12}  "),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(display_name),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("Displayed: {display_count}/{total_entries} | Listed uncompressed: {unpacked_total} bytes | Compressed: {compressed_total} bytes"),
        Style::default().fg(Color::Green),
    )));

    if total_entries > display_count {
        lines.push(Line::from(Span::styled(
            format!("Showing first {MAX_ARCHIVE_ENTRIES_TO_DISPLAY} entries to keep the preview responsive."),
            Style::default().fg(Color::Yellow),
        )));
    }

    lines.push(Line::from("Read-only preview. No files are extracted."));
    Preview::Archive(lines)
}

fn make_preview(
    path: &Path,
    picker: &mut Picker,
    syntax_set: &SyntaxSet,
    theme_set: &ThemeSet,
) -> Preview {
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) => return Preview::Info(format!("Cannot read file: {e}")),
    };

    if path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
    {
        return make_zip_preview(path, metadata.len());
    }

    if is_image(path) {
        if metadata.len() > 50 * 1024 * 1024 {
            return Preview::Info(format!(
                "Image is larger than 50 MB.\n\nName: {}\nSize: {} bytes",
                path.display(),
                metadata.len()
            ));
        }

        return match image::ImageReader::open(path)
            .context("Cannot open image")
            .and_then(|reader| reader.decode().context("Cannot decode image"))
        {
            Ok(img) => {
                let (width, height) = (img.width(), img.height());
                let info = format!(
                    "{} × {} pixels | {} bytes",
                    width,
                    height,
                    metadata.len()
                );

                Preview::Image(picker.new_resize_protocol(img), info)
            }
            Err(e) => Preview::Info(format!("Image preview failed:\n{e}")),
        };
    }

    // Avoid loading extremely large files into memory.
   
    const MAX_TEXT_SIZE: u64 = 2 * 1024 * 1024;

    if metadata.len() > MAX_TEXT_SIZE {
      return match read_text_page(path, 0, 1) {
          Ok(mut page) => {
             highlight_page(
                &mut page,
                path,
                syntax_set,
                theme_set,
             );

             Preview::LargeText {
                pages: VecDeque::from([page]),
                current: 0,
             }
          }
          Err(e) => Preview::Info(format!(
              "Cannot preview large file:\n{e}"
          )),
      };
    }

    let mut bytes = Vec::new();

    if let Err(e) = fs::File::open(path)
        .and_then(|file| file.take(MAX_TEXT_SIZE + 1).read_to_end(&mut bytes))
    {
        return Preview::Info(format!("Cannot read file: {e}"));
    }

    if bytes.contains(&0) {
        return Preview::Info(format!(
            "Binary file\n\nName: {}\nSize: {} bytes\n\nThis file cannot be displayed as text.",
            path.display(),
            metadata.len()
        ));
    }

    let content = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => {
            return Preview::Info(
                "This file is not valid UTF-8 text.".into()
            );
        }
    };

    let syntax = syntax_set
        .find_syntax_for_file(path)
        .ok()
        .flatten()
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

    let theme = match theme_set.themes.get("base16-ocean.dark") {
        Some(t) => t,
        None => return Preview::Info("Syntax theme unavailable".into()),
    };

    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut lines = Vec::new();

    for (number, line) in LinesWithEndings::from(&content).enumerate() {
        let mut spans = vec![Span::styled(
            format!("{:>4} ", number + 1),
            Style::default().fg(Color::DarkGray),
        )];

        if let Ok(ranges) = highlighter.highlight_line(line, syntax_set) {
            for (style, text) in ranges {
                spans.push(Span::styled(
                    text.to_string(),
                    Style::default().fg(Color::Rgb(
                        style.foreground.r,
                        style.foreground.g,
                        style.foreground.b,
                    )),
                ));
            }
        }

        lines.push(Line::from(spans));
    }

    Preview::Text(lines)
    
}


fn search_directory(
    directory: &Path,
    query: &str,
    results: &mut Vec<String>,
) -> Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();

        // Skip hidden files and directories.
        if name.starts_with('.') {
            continue;
        }

        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };

        // Never follow symbolic links.
        if metadata.file_type().is_symlink() {
            continue;
        }

        if metadata.is_dir() {
            search_directory(&path, query, results)?;
        } else if metadata.is_file() {
            // Reuse CyberClip's existing search engine.
            if let Ok(found) = crate::get::search(&path, query, true) {
                for line in found.text.lines() {
                    let line = line.trim_start();

                    if let Some((number, content)) = line.split_once("  ") {
                        results.push(format!(
                            "{}:{}: {}",
                            path.display(),
                            number,
                            content
                        ));
                    }
                }
            }
        }
    }

    Ok(())
}

pub fn run() -> Result<()> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut picker =
        Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());

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
    let mut current_dir = std::env::current_dir()?;
    let mut entries = read_directory(&current_dir)?;
    let mut selected = 0usize;
    let mut scroll = 0u16;
    let mut search = String::new();
    let mut searching = false;
    let mut preview_path: Option<PathBuf> = None;
    let mut cached_preview: Option<(PathBuf, Preview)> = None;
    let mut message = String::from("Read-only file explorer");
    let mut content_matches: Vec<String> = Vec::new();
    let mut content_match_index = 0usize;
    let mut content_search_active = false;
    let mut content_prompt_active = false;
    let mut content_query = String::new();

    let syntax_set = SyntaxSet::load_defaults_newlines();
    let theme_set = ThemeSet::load_defaults();

    loop {
        let visible_entries: Vec<&FileEntry> = entries
            .iter()
            .filter(|entry| {
                search.is_empty()
                    || entry.name.to_lowercase().contains(&search.to_lowercase())
            })
            .collect();

        selected = if visible_entries.is_empty() {
            0
        } else {
            selected.min(visible_entries.len() - 1)
        };

        // Build the preview only when a different file is opened.
        if let Some(path) = &preview_path {
            let needs_refresh = cached_preview
                .as_ref()
                .map(|(cached_path, _)| cached_path != path)
                .unwrap_or(true);

            if needs_refresh {
                let preview = make_preview(
                    path,
                    picker,
                    &syntax_set,
                    &theme_set,
                );

                cached_preview = Some((path.clone(), preview));
            }
        }

        terminal.draw(|frame| {
            let area = frame.area();

            let sections = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(5),
                    Constraint::Length(3),
                ])
                .split(area);

            let header = Paragraph::new(format!(
                " CYBERCLIP // FILE EXPLORER // {}",
                current_dir.display()
            ))
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .block(Block::default().borders(Borders::ALL));

            frame.render_widget(header, sections[0]);

            let panels = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(42),
                    Constraint::Percentage(58),
                ])
                .split(sections[1]);

            let items: Vec<ListItem> = visible_entries
                .iter()
                .map(|entry| {
                    let icon = file_icon(entry);
                    ListItem::new(format!("{} {}", icon, entry.name))
                })
                .collect();

            let mut state = ListState::default();

            if !visible_entries.is_empty() {
                state.select(Some(selected));
            }

            let list = List::new(items)
                .block(
                    Block::default()
                        .title(format!(" FILES ({}) ", visible_entries.len()))
                        .borders(Borders::ALL),
                )
                .highlight_style(
                    Style::default()
                        .bg(Color::Rgb(20, 55, 65))
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("▶ ");

            frame.render_stateful_widget(list, panels[0], &mut state);

            let preview_block = Block::default()
                .title(if content_search_active {
                   " CONTENT SEARCH RESULTS "
                } else {
                   " FILE PREVIEW "
                })
                .borders(Borders::ALL);
                

                

            let preview_area = preview_block.inner(panels[1]);
            frame.render_widget(preview_block, panels[1]);

            if content_search_active {
               if content_matches.is_empty() {
                  frame.render_widget(
                     Paragraph::new("No matches found."),
                     preview_area,
                  );
               } else {
                  let result_lines: Vec<Line> = content_matches
                     .iter()
                     .enumerate()
                     .map(|(index, result)| {
                        let prefix = if index == content_match_index {
                           "▶ "
                        } else {
                           "  "
                        };

                        Line::from(format!("{prefix}{result}"))
                     })
                     .collect();

                     frame.render_widget(
                        Paragraph::new(result_lines)
                           .scroll((content_match_index as u16, 0))
                           .wrap(Wrap { trim: false }),
                        preview_area,
                     );
                  }
               } else if let Some((_, preview)) = cached_preview.as_mut() {
               match preview {
                    Preview::Text(lines) | Preview::Archive(lines) => {
                        frame.render_widget(
                            Paragraph::new(Text::from(lines.clone()))
                                .scroll((scroll, 0))
                                .wrap(Wrap { trim: false }),
                            preview_area,
                        );
                    }
                    Preview::LargeText { pages, current } => {
                       if let Some(page) = pages.get(*current) {
                          let visible_lines: Vec<Line> = page
                             .highlighted
                             .iter()
                             .skip(scroll as usize)
                             .take(preview_area.height as usize)
                             .cloned()
                             .collect();

                          frame.render_widget(
                             Paragraph::new(visible_lines),
                             preview_area,
                          );
                       }
                    }

                    Preview::Image(protocol, info) => {
                        frame.render_stateful_widget(
                            StatefulImage::default(),
                            preview_area,
                            protocol,
                        );

                        let info_area = Layout::default()
                            .direction(Direction::Vertical)
                            .constraints([
                                Constraint::Min(1),
                                Constraint::Length(1),
                            ])
                            .split(preview_area);

                        frame.render_widget(
                            Paragraph::new(info.as_str())
                                .style(Style::default().fg(Color::Green)),
                            info_area[1],
                        );
                    }

                    Preview::Info(info) => {
                        frame.render_widget(
                            Paragraph::new(info.as_str())
                                .wrap(Wrap { trim: false }),
                            preview_area,
                        );
                    }
               }
            } else if let Some(entry) = visible_entries.get(selected) {
                let details = format!(
                    "Name: {}\n\nPath: {}\n\nType: {}\n\nSize: {} bytes\n\n\nPress ENTER to open.",
                    entry.name,
                    entry.path.display(),
                    if entry.is_dir { "Directory" } else { "File" },
                    entry.size
                );

                frame.render_widget(
                    Paragraph::new(details)
                        .wrap(Wrap { trim: false }),
                    preview_area,
                );
            } else {
                frame.render_widget(
                    Paragraph::new("This directory is empty."),
                    preview_area,
                );
            }

            let footer_text = if content_prompt_active {
               format!(
                  "CONTENT SEARCH: {} | Enter Search | Esc Cancel",
                  content_query
               )
            } else if content_search_active {
               format!(
                  "{} | N Next | P Previous | C Copy All | Esc Close",
                  message
               )
            } else {
               format!(
                  "{} | Filename Search: {} | F Content Search | ↑↓ Navigate | Enter Open | Backspace Back | / Search | PgUp/PgDn Scroll | Q Quit",
                  message, search
               )
            };

            let footer = Paragraph::new(footer_text)
               .style(Style::default().fg(Color::Green))
               .block(Block::default().borders(Borders::ALL));

            frame.render_widget(footer, sections[2]);
        })?;

        if !event::poll(Duration::from_millis(40))? {
            continue;
        }

        let Event::Key(key) = event::read()? else {
            continue;
        };

        if key.kind != KeyEventKind::Press {
            continue;
        }

        if content_prompt_active {
           match key.code {
              KeyCode::Esc => {
                 content_prompt_active = false;
              }

              KeyCode::Enter => {
                 content_prompt_active = false;
                 content_matches.clear();
                 content_match_index = 0;

                 if content_query.trim().is_empty() {
                    message = "Search query cannot be empty".into();
                 } else {
                    match search_directory(
                       &current_dir,
                       &content_query,
                       &mut content_matches,
                    ) {
                    Ok(()) => {
                        content_search_active = true;
                        message = format!(
                            "{} content match(es) found",
                            content_matches.len()
                        );
                    }
                    Err(e) => {
                        message = format!("Search error: {e}");
                    }
                 }
              }
           }

           KeyCode::Backspace => {
              content_query.pop();
           }

           KeyCode::Char(c) => {
              content_query.push(c);
           }

           _ => {}
        }

        continue;
}

        if searching {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => searching = false,
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

            KeyCode::Char('f') | KeyCode::Char('F') => {
               content_query.clear();
               content_prompt_active = true;
               content_search_active = false;
            }

            KeyCode::Char('n') | KeyCode::Char('N')
               if content_search_active =>
            {
               if !content_matches.is_empty() {
                  content_match_index =
                     (content_match_index + 1) % content_matches.len();
               }
            }

            KeyCode::Char('p') | KeyCode::Char('P')
               if content_search_active =>
            {
               if !content_matches.is_empty() {
                  content_match_index = if content_match_index == 0 {
                     content_matches.len() - 1
                  } else {
                     content_match_index - 1
                  };
               }
            }

            KeyCode::Char('c') | KeyCode::Char('C')
               if content_search_active =>
            {
               if !content_matches.is_empty() {
                  let copied = content_matches.join("\n");

                  crate::clipboard(copied.as_bytes(), None)?;

                  message = format!(
                     "{} search result(s) copied",
                     content_matches.len()
                  );
               }
            }

            KeyCode::Esc if content_search_active => {
               content_search_active = false;
            }
            KeyCode::Char('q') | KeyCode::Esc => break,

            KeyCode::Down => {
                if selected + 1 < visible_entries.len() {
                    selected += 1;
                }
            }

            KeyCode::Up => {
                selected = selected.saturating_sub(1);
            }

            KeyCode::PageDown => {
               let mut loaded_next = false;

               if let Some((_, Preview::LargeText { pages, current })) =
                  cached_preview.as_mut()
               {
                  if let Some(page) = pages.get(*current) {
                     if scroll as usize + 10 < page.lines.len() {
                        scroll = scroll.saturating_add(10);
                     } else if *current + 1 < pages.len() {
                        *current += 1;
                        scroll = 0;
                     } else if !page.eof {
                        let offset = page.next_offset;
                        let first_line = page.first_line + page.lines.len() as u64;

                        if let Some(path) = preview_path.as_ref() {
                           match read_text_page(path, offset, first_line) {
                              Ok(mut next_page) => {
                                 highlight_page(
                                    &mut next_page,
                                    path,
                                    &syntax_set,
                                    &theme_set,
                                 );

                                 if pages.len() == 3 {
                                    pages.pop_front();
                                 } else {
                                    *current += 1;
                                 }

                                 pages.push_back(next_page);
                                 *current = pages.len() - 1;
                                 scroll = 0;
                                 loaded_next = true;
                              }
                              Err(e) => message = format!("Read error: {e}"),
                           }
                        }
                     }
                  }
               } else {
                  scroll = scroll.saturating_add(10);
               }

               if loaded_next {
                  message = "Next file section loaded".into();
               }
            }

            KeyCode::PageUp => {
               if let Some((_, Preview::LargeText { current, .. })) =
                  cached_preview.as_mut()
               {
                  if scroll >= 10 {
                     scroll = scroll.saturating_sub(10);
                  } else if *current > 0 {
                     *current -= 1;
                     scroll = 0;
                  }
               } else {
                  scroll = scroll.saturating_sub(10);
               }
            }

            KeyCode::Char('/') => {
                search.clear();
                selected = 0;
                searching = true;
            }

            KeyCode::Backspace => {
                if preview_path.is_some() {
                    preview_path = None;
                    cached_preview = None;
                    scroll = 0;
                } else if let Some(parent) = current_dir.parent() {
                    current_dir = parent.to_path_buf();
                    entries = read_directory(&current_dir)?;
                    selected = 0;
                    search.clear();
                }
            }

            KeyCode::Enter => {
                if let Some(entry) = visible_entries.get(selected) {
                    if entry.is_dir {
                        current_dir = entry.path.clone();
                        entries = read_directory(&current_dir)?;
                        selected = 0;
                        preview_path = None;
                        cached_preview = None;
                        scroll = 0;
                        search.clear();
                        message = "Directory opened".into();
                    } else {
                        preview_path = Some(entry.path.clone());
                        cached_preview = None;
                        scroll = 0;
                        message = "Preview cached".into();
                    }
                }
            }

            _ => {}
        }
    }

    Ok(())
}

