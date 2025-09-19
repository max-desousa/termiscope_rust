use crossterm::{
    cursor::{MoveTo, Show},
    event::{poll, read, Event, KeyCode, KeyEvent},
    style::{Color, Print, SetForegroundColor, ResetColor},
    terminal::{self, Clear, ClearType, size},
    ExecutableCommand,
};
use regex::{Regex, RegexBuilder};
use std::fs;
use std::io::{stdout, Write};
use std::path::Path;
use std::time::Duration;
use walkdir::{ WalkDir, DirEntry };

fn main() -> std::io::Result<()> {
    // Enable raw mode to capture key events
    terminal::enable_raw_mode()?;
    let mut stdout = stdout();

    // Clear the terminal initially
    stdout.execute(Clear(ClearType::All))?.execute(MoveTo(0, 0))?;

    let mut query = String::new();
    let files = collect_text_files();
    let mut last_results: Vec<(String, String)> = Vec::new(); // Store (file, match) pairs
    let mut current_results: Vec<(String, String)> = Vec::new(); // Current search results
    let mut results_start_row = 2; // Row where current results start
    let (terminal_width, terminal_height) = size()?; // Terminal width for alignment
    let terminal_width = terminal_width as usize;

    // Initial prompt
    stdout
        .execute(MoveTo(0, 0))?
        .execute(Print("Search: "))?;
    stdout.flush()?;

    loop {
        // Update query display only if changed
        stdout
            .execute(MoveTo(8, 0))? // Move to after "Search: "
            .execute(Print(&query))?
            .execute(Print(" ".repeat(50)))?; // Clear leftover query text

        // Update results only if they changed
        let new_results = search_file_contents(&files, &query);
        if new_results != current_results {
            current_results = new_results;

            // Clear only the current results area
            for i in 0..(terminal_height - 3) {
                stdout
                    .execute(MoveTo(0, results_start_row + i as u16))?
                    .execute(Print(" ".repeat(terminal_width)))?;
            }

            // Display current search results
            for (i, (file, matched_str)) in current_results.iter().take((terminal_height - 3) as usize).enumerate() {
                // Truncate file path if too long
                let max_file_len = terminal_width.saturating_sub(matched_str.len() + 5);
                let display_file = if file.len() > max_file_len {
                    format!("...{}", &file[file.len().saturating_sub(max_file_len - 3)..])
                } else {
                    file.to_string()
                };

                // Align matched string to the right in orange
                let padding = terminal_width.saturating_sub(display_file.len() + matched_str.len());
                stdout
                    .execute(MoveTo(0, results_start_row + i as u16))?
                    .execute(Print(&display_file))?
                    .execute(Print(" ".repeat(padding)))?
                    .execute(SetForegroundColor(Color::DarkYellow))?
                    .execute(Print(matched_str))?
                    .execute(ResetColor)?;
            }
        }

        stdout.flush()?;

        // Poll for keyboard events
        if poll(Duration::from_millis(100))? {
            if let Event::Key(KeyEvent { code, .. }) = read()? {
                match code {
                    KeyCode::Esc => break, // Exit on ESC
                    KeyCode::Enter => {
                        // Save current results and clear query
                        last_results = current_results.clone();
                        query.clear();
                        // Move results area down
                        results_start_row = last_results.len() as u16 + 3;
                        // Clear current results area for new search
                        for i in 0..(terminal_height - 3) {
                            stdout
                                .execute(MoveTo(0, results_start_row + i as u16))?
                                .execute(Print(" ".repeat(terminal_width)))?;
                        }
                        // Move prompt below last results
                        stdout
                            .execute(MoveTo(0, results_start_row - 1))?
                            .execute(Print("Search: "))?;
                        current_results.clear(); // Reset current results
                    }
                    KeyCode::Backspace => {
                        query.pop(); // Remove last character
                    }
                    KeyCode::Char(c) => {
                        query.push(c); // Add typed character
                    }
                    _ => {}
                }
            }
        }
    }

    // Cleanup: disable raw mode and show cursor
    terminal::disable_raw_mode()?;
    stdout
        .execute(MoveTo(0, terminal_height))?
        .execute(Show)?;
    Ok(())
}

fn is_not_hidden(entry : &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| !s.starts_with('.'))
        .unwrap_or(true)
}

// Collect all non-binary (text) files in the current directory and subdirectories
fn collect_text_files() -> Vec<String> {
    let mut files = Vec::new();
    for entry in WalkDir::new(".")
        .into_iter()
        //.filter_entry(|e|  is_not_hidden(e))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
    {
        let path = entry.path();
        if is_text_file(path) {
            if let Some(path_str) = path.to_str() {
                files.push(path_str.to_string());
            }
        }
    }
    files
}

// Common text file extensions suitable for grep
const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "md", "rs", "py", "js", "ts", "html", "css", "json", "yaml", "yml",
    "toml", "ini", "sh", "bash", "cpp", "c", "h", "java", "go", "rb", "php", "sql",
];

fn is_text_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| TEXT_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

// Search file contents for matches based on query as a regex pattern
fn search_file_contents(files: &[String], query: &str) -> Vec<(String, String)> {
    if query.is_empty() {
        return files.iter().map(|f| (f.clone(), "".to_string())).collect();
    }

    // Use RegexBuilder for case-insensitive regex
    let re = match RegexBuilder::new(query)
        .case_insensitive(true)
        .build()
    {
        Ok(regex) => regex,
        Err(_) => {
            // Fallback to empty results or a special "invalid regex" result
            return vec![("".to_string(), "Invalid regex pattern".to_string())];
        }
    };

    let mut matches = Vec::new();

    for file in files {
        if let Ok(content) = fs::read_to_string(file) {
            for line in content.lines() {
                if re.is_match(line) {
                    // Truncate matched line if too long
                    let matched_line = if line.len() > 50 {
                        format!("{}...", &line[..47])
                    } else {
                        line.to_string()
                    };
                    matches.push((file.clone(), matched_line));
                }
            }
        }
    }

    matches
}
