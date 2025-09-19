use std::io::{Write, stdout};
use crossterm::{
    terminal::{
    Clear,
    ClearType,
    enable_raw_mode,
    disable_raw_mode,
    },
    event::{
        self,
        Event,
        KeyEvent,
        KeyCode,
        KeyModifiers,
    },
    style::Print,
    QueueableCommand,
    cursor,
};
fn main() -> std::io::Result<()> {
    let mut stdout = stdout();
    enable_raw_mode()?;
    let (size_columns, size_rows) = crossterm::terminal::size()?;
    let dimension_string = format!(
        "Terminal size: {} columns, {} rows",
        size_columns, size_rows
    );
    stdout
        .queue(Clear(ClearType::All))?
        .queue(cursor::MoveTo(0, 0))?
        .queue(Print("Hello, world! - Welcome to termiscope!\r\n"))?
        .queue(Print(dimension_string))?;
    stdout.flush()?;
    let mut input_buffer = String::new();
    loop {
        if let Event::Key(key_event) = event::read()? {
            match key_event {
                KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                } => {
                    stdout
                        .queue(Clear(ClearType::All))?
                        .queue(cursor::MoveTo(0, 0))?
                        .queue(Print("Ctrl+C detected, exiting..."))?;
                    stdout.flush()?;
                    break;
                }
                KeyEvent {
                    code: KeyCode::Backspace,
                    ..
                } => {
                    if !input_buffer.is_empty() {
                        input_buffer.pop();
                        let message = format!(
                            "Buffer is now: {}",
                            input_buffer);
                        stdout
                            .queue(Clear(ClearType::CurrentLine))?
                            .queue(cursor::MoveTo(0, 0))?
                            .queue(Print(message))?;
                        stdout.flush()?;
                    } else {
                        let message = format!(
                            "Buffer is now: {} (hitting backspace doesn't matter dummy)",
                            input_buffer);
                        stdout
                            .queue(Clear(ClearType::CurrentLine))?
                            .queue(cursor::MoveTo(0, 0))?
                            .queue(Print(message))?;
                        stdout.flush()?;
                    }
                }
                _ => {
                    if let Some(char) = key_event.code.as_char() {
                        input_buffer.push_str(&char.to_string());
                        let message = format!(
                            "Buffer is now: {}",
                            input_buffer);
                        stdout
                            .queue(Clear(ClearType::CurrentLine))?
                            .queue(cursor::MoveTo(0, 0))?
                            .queue(Print(message))?;
                        stdout.flush()?;
                    }
                }
            }
        }
    }
    disable_raw_mode()?;
    Ok(())
}
