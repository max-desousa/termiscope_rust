use crossterm::{
    cursor::{MoveTo, Show},
    event::{poll, read, Event, KeyCode, KeyEvent},
    style::{Color, Print, SetForegroundColor, ResetColor},
    terminal::{self, Clear, ClearType, size},
    ExecutableCommand,
};
use lru::LruCache;
use regex::RegexBuilder;
use std::fs;
use std::io::{stdout, Write};
use std::num::NonZeroUsize;
use std::path::Path;
use std::time::Duration;
use walkdir::{WalkDir, DirEntry};
use clap::Parser;

/// Simple dynamic grep tool emulating neovim's telescope plugin
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Case sensitivity of the regular expression search
    #[arg(short, long, default_value_t = false)]
    insensitive_to_case : bool,

    /// Extensions of files to search
    #[arg(short, long, value_delimiter = ',', num_args = 0..)]
    extensions : Option<Vec<String>>,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    // Enable raw mode to capture key events
    terminal::enable_raw_mode()?;
    let mut stdout = stdout();

    // Clear the terminal initially
    stdout.execute(Clear(ClearType::All))?.execute(MoveTo(0, 0))?;

    let mut query = String::new();
    let files = collect_text_files(&args);
    let mut content_cache = LruCache::new(NonZeroUsize::new(100).expect("Cache size must be non-zero"));
    let mut current_results: Vec<(String, String, Vec<(usize, usize)>)> = Vec::new();
    let mut results_start_row = 2;
    let (terminal_width, terminal_height) = size()?;
    let terminal_width = terminal_width as usize;

    // Initial prompt
    stdout
        .execute(MoveTo(0, 0))?
        .execute(Print("Search: "))?;
    stdout.flush()?;

    loop {
        // Update query display and position cursor at end of query
        stdout
            .execute(MoveTo(8, 0))? // After "Search: "
            .execute(Print(&query))?
            .execute(Print(" ".repeat(50)))? // Clear leftover text
            .execute(MoveTo(8 + query.len() as u16, 0))?; // Move cursor to end of query

        // Update results if changed
        let new_results = search_file_contents(&files, &query, &mut content_cache, terminal_width, &args);
        if new_results != current_results {
            current_results = new_results;

            // Clear results area
            for i in 0..(terminal_height - 3) {
                stdout
                    .execute(MoveTo(0, results_start_row + i as u16))?
                    .execute(Print(" ".repeat(terminal_width)))?;
            }

            // Display results (limited to terminal_height - 3)
            for (i, (file, matched_str, match_ranges)) in current_results
                .iter()
                .take((terminal_height - 3) as usize)
                .enumerate()
            {
                // Handle invalid regex
                if file.is_empty() && matched_str == "Invalid regex pattern" {
                    stdout
                        .execute(MoveTo(0, results_start_row + i as u16))?
                        .execute(SetForegroundColor(Color::Red))?
                        .execute(Print(matched_str))?
                        .execute(ResetColor)?;
                    continue;
                }

                // Truncate file path (max 30 chars)
                let max_file_len = 30.min(terminal_width / 2);
                let display_file = if file.len() > max_file_len {
                    format!("...{}", &file[file.len().saturating_sub(max_file_len - 3)..])
                } else {
                    file.to_string()
                };

                // Render file path
                stdout
                    .execute(MoveTo(0, results_start_row + i as u16))?
                    .execute(SetForegroundColor(Color::White))?
                    .execute(Print(&display_file))?
                    .execute(ResetColor)?;

                // Calculate padding
                let padding = terminal_width.saturating_sub(display_file.len() + matched_str.len());
                stdout.execute(Print(" ".repeat(padding)))?;

                // Render matched string
                let mut last_pos = 0;
                for &(start, end) in match_ranges {
                    if start > last_pos {
                        stdout
                            .execute(SetForegroundColor(Color::Cyan))?
                            .execute(Print(&matched_str[last_pos..start]))?;
                    }
                    stdout
                        .execute(SetForegroundColor(Color::Magenta))?
                        .execute(Print(&matched_str[start..end]))?;
                    last_pos = end;
                }
                if last_pos < matched_str.len() {
                    stdout
                        .execute(SetForegroundColor(Color::Cyan))?
                        .execute(Print(&matched_str[last_pos..]))?;
                }
                stdout.execute(ResetColor)?;
            }
        }

        stdout.flush()?;

        // Poll for keyboard events
        if poll(Duration::from_millis(100))? {
            if let Event::Key(KeyEvent { code, .. }) = read()? {
                match code {
                    KeyCode::Esc => break,
                    KeyCode::Enter => {
                        let last_results_len = current_results.len();
                        query.clear();
                        results_start_row = last_results_len as u16 + 3;
                        for i in 0..(terminal_height - 3) {
                            stdout
                                .execute(MoveTo(0, results_start_row + i as u16))?
                                .execute(Print(" ".repeat(terminal_width)))?;
                        }
                        stdout
                            .execute(MoveTo(0, results_start_row - 1))?
                            .execute(Print("Search: "))?;
                        current_results.clear();
                    }
                    KeyCode::Backspace => {
                        query.pop();
                    }
                    KeyCode::Char(c) => {
                        query.push(c);
                    }
                    _ => {}
                }
            }
        }
    }

    // Cleanup: disable raw mode, position cursor dynamically, show cursor
    terminal::disable_raw_mode()?;
    let exit_row = if current_results.len() >= (terminal_height - 3) as usize {
        // Many results: use terminal_height - 2 (leaves one blank line)
        terminal_height - 2
    } else {
        // Few results: use row after last result
        results_start_row + current_results.len() as u16
    };
    stdout
        .execute(MoveTo(0, exit_row))?
        .execute(Show)?;
    Ok(())
}

fn is_not_hidden(entry: &DirEntry) -> bool {
    if entry.file_type().is_dir() {
        true
    } else {
        entry
            .file_name()
            .to_str()
            .map(|s| !s.starts_with('.'))
            .unwrap_or(true)
    }
}

fn collect_text_files(args : &Args) -> Vec<String> {
    let mut files = Vec::new();
    for entry in WalkDir::new(".")
        .into_iter()
        .filter_entry(|e| is_not_hidden(e))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
    {
        let path = entry.path();
        if is_text_file(path, args) {
            if let Some(path_str) = path.to_str() {
                files.push(path_str.to_string());
            }
        }
    }
    files
}

const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "md", "rs", "py", "js", "ts", "html", "css", "json", "yaml", "yml", "toml", "ini", "sh",
    "bash", "cpp", "c", "h", "java", "go", "rb", "php", "sql",
];

fn is_text_file(path: &Path, args : &Args) -> bool {
    let extensions_to_use: Vec<String> = args
            .extensions
            .clone()
            .unwrap_or_else(|| TEXT_EXTENSIONS.iter().map(|&s| s.to_string()).collect());

    // Check if the file's extension is in the list
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| extensions_to_use.iter().any(|e| e.to_lowercase() == ext.to_lowercase()))
        .unwrap_or(false)
}

fn search_file_contents(
    files: &[String],
    query: &str,
    content_cache: &mut LruCache<String, String>,
    terminal_width: usize,
    args : &Args
) -> Vec<(String, String, Vec<(usize, usize)>)> {
    if query.is_empty() {
        return files
            .iter()
            .map(|f| (f.clone(), "".to_string(), vec![]))
            .collect();
    }

    let re = match RegexBuilder::new(query)
        .case_insensitive(args.insensitive_to_case)
        .build()
    {
        Ok(regex) => regex,
        Err(_) => {
            return vec![("".to_string(), "Invalid regex pattern".to_string(), vec![])];
        }
    };

    let mut matches = Vec::new();

    for file in files {
        let content = if let Some(content) = content_cache.get(file) {
            content.clone()
        } else {
            match fs::read_to_string(file) {
                Ok(content) => {
                    content_cache.put(file.clone(), content.clone());
                    content
                }
                Err(_) => continue,
            }
        };

        for line in content.lines() {
            let mut match_ranges = vec![];
            let mut first_match_start = None;
            for mat in re.find_iter(line) {
                if first_match_start.is_none() {
                    first_match_start = Some(mat.start());
                }
                match_ranges.push((mat.start(), mat.end()));
            }
            if !match_ranges.is_empty() {
                // Initialize truncation variables
                let max_text_len = terminal_width.saturating_sub(33); // 30 for path + 3 for padding
                let start_pos;
                let prefix_offset;
                let matched_line = if line.len() > max_text_len {
                    let start = first_match_start.unwrap_or(0);
                    let context = 20.min(start); // Up to 20 chars before match
                    start_pos = start.saturating_sub(context);
                    let end_pos = (start_pos + max_text_len).min(line.len());
                    let mut truncated = line[start_pos..end_pos].to_string();
                    prefix_offset = if start_pos > 0 {
                        truncated = format!("...{}", truncated);
                        3 // Account for "..."
                    } else {
                        0
                    };
                    if end_pos < line.len() {
                        truncated.push_str("...");
                    }
                    truncated
                } else {
                    start_pos = 0;
                    prefix_offset = 0;
                    line.to_string()
                };

                // Adjust match ranges for truncated line
                let adjusted_ranges = match_ranges
                    .into_iter()
                    .filter(|&(start, _)| start >= start_pos) // Include ranges after start_pos
                    .map(|(start, end)| {
                        let new_start = start - start_pos + prefix_offset;
                        let new_end = end - start_pos + prefix_offset;
                        (new_start, new_end.min(matched_line.len()))
                    })
                    .filter(|&(start, end)| start < matched_line.len() && end <= matched_line.len())
                    .collect::<Vec<(usize, usize)>>();

                matches.push((file.clone(), matched_line, adjusted_ranges));
            }
        }
    }

    matches
}
