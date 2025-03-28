use crossterm::{
    cursor,
    event::{self, Event, KeyCode},
    execute, queue,
    style::{self, Stylize},
    terminal,
};
use std::io::{self, Write};
use unicode_width::UnicodeWidthChar;

use crate::{Error, Row};

#[derive(Debug, Clone, Copy, Default)]
pub struct Position {
    pub x: usize,
    pub y: usize,
}

#[derive(Default)]
pub struct Editor {
    buffer: Vec<Row>,
    debug_output: String,
    height: usize,
    width: usize,

    sidebar_width: usize,

    offset: Position,
    cursor: Position,
}

impl Editor {
    pub fn new() -> Self {
        Self::default()
    }

    fn get_width(&self) -> usize {
        self.buffer[self.cursor.y].len()
    }

    pub fn init(&mut self, filename: &Option<String>) -> Result<(), Error> {
        let mut stdout = io::stdout();

        if let Some(name) = filename {
            self.buffer = std::fs::read_to_string(name)?
                .split('\n')
                .map(|s| Row::from(s.to_string()))
                .collect();
        } else {
            self.buffer = Vec::new();
            self.buffer.push(Row::from(String::new()));
        }

        terminal::enable_raw_mode()?;
        execute!(
            stdout,
            // terminal::EnterAlternateScreen,
            // terminal::DisableLineWrap,
            event::EnableMouseCapture,
            event::EnableBracketedPaste,
            event::EnableFocusChange,
        )?;

        let (width, height) = terminal::size()?;
        (self.height, self.width) = (height as usize, width as usize);

        self.render()?;
        self.event_loop()?;

        self.on_exit()?;

        Ok(())
    }

    fn event_loop(&mut self) -> Result<(), Error> {
        loop {
            if event::poll(std::time::Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(event) => match event.code {
                        KeyCode::Esc => {
                            // TODO: Confirm exit
                            break;
                        }

                        KeyCode::Down => {
                            if self.cursor.y < self.buffer.len() - 1 {
                                self.cursor.y += 1;
                            } else {
                                self.cursor.x = self.get_width();
                            }
                        }
                        KeyCode::Up => {
                            if self.cursor.y > 0 {
                                self.cursor.y -= 1;
                            } else {
                                self.cursor.x = 0;
                            }
                        }
                        KeyCode::Left => {
                            self.cursor.x = self.cursor.x.min(self.get_width());
                            if self.cursor.x > 0 {
                                self.cursor.x -= 1;
                            } else if self.cursor.y > 0 {
                                self.cursor.y -= 1;
                                self.cursor.x = self.get_width();
                            }
                        }
                        KeyCode::Right => {
                            self.cursor.x = self.cursor.x.min(self.get_width());
                            if self.cursor.x < self.get_width() {
                                self.cursor.x += 1;
                            } else if self.cursor.y < self.buffer.len() - 1 {
                                self.cursor.y += 1;
                                self.cursor.x = 0;
                            }
                        }

                        KeyCode::PageUp => {
                            self.cursor.y = self.cursor.y.saturating_sub(self.height - 2);
                        }
                        KeyCode::PageDown => {
                            self.cursor.y =
                                (self.cursor.y + self.height - 2).min(self.buffer.len() - 1);
                        }
                        KeyCode::Home => {
                            self.cursor.x = 0;
                        }
                        KeyCode::End => {
                            self.cursor.x = self.get_width();
                        }

                        KeyCode::Enter => {
                            let new_line = Row {
                                rope: self.buffer[self.cursor.y].rope[self.cursor.x..].to_vec(),
                            };
                            self.buffer.insert(self.cursor.y + 1, new_line);
                            self.buffer[self.cursor.y] = Row {
                                rope: self.buffer[self.cursor.y].rope[..self.cursor.x].to_vec(),
                            };
                            self.cursor.y += 1;
                            self.cursor.x = 0;
                        }

                        KeyCode::Backspace => {
                            if self.cursor.x > 0 {
                                self.cursor.x -= 1;
                                self.buffer[self.cursor.y].rope.remove(self.cursor.x);
                            } else if self.cursor.y > 0 {
                                self.cursor.y -= 1;
                                self.cursor.x = self.get_width();

                                let mut rope = self.buffer[self.cursor.y].rope.clone();
                                rope.extend(self.buffer.remove(self.cursor.y).rope);
                                self.buffer[self.cursor.y] = Row { rope };
                            }
                        }

                        KeyCode::Delete => {
                            if self.cursor.x < self.get_width() {
                                self.buffer[self.cursor.y].rope.remove(self.cursor.x);
                            } else if self.cursor.y < self.buffer.len() - 1 {
                                let mut rope = self.buffer[self.cursor.y].rope.clone();
                                rope.extend(self.buffer.remove(self.cursor.y + 1).rope);
                                self.buffer[self.cursor.y] = Row { rope };
                            }
                        }

                        KeyCode::Char(c) => {
                            self.buffer[self.cursor.y]
                                .rope
                                .insert(self.cursor.x, (c.to_string(), c.width().unwrap_or(0)));
                            self.cursor.x += 1;
                        }

                        _ => {}
                    },
                    Event::Mouse(event) => {
                        // TODO: Handle mouse events
                    }

                    Event::Resize(width, height) => {
                        (self.height, self.width) = (height as usize, width as usize);
                    }
                    _ => {}
                }

                if self.width < 5 || self.height < 5 {
                    continue;
                }

                self.update_offset();

                let c = self.get_cursor_position();
                self.debug_output = format!(
                    "View: ({}, {}) | Cursor: ({}, {}) / ({}, {})",
                    self.offset.y + 1,
                    self.offset.x + 1,
                    self.cursor.y + 1,
                    self.cursor.x + 1,
                    c.y + 1,
                    c.x + 1
                );

                self.render()?;
            }
        }

        Ok(())
    }

    fn render(&mut self) -> Result<(), Error> {
        self.update_sidebar_width();

        let cursor = self.get_cursor_position();

        let mut stdout = io::stdout();

        execute!(
            stdout,
            terminal::BeginSynchronizedUpdate,
            cursor::Hide,
            terminal::Clear(terminal::ClearType::All),
        )?;

        // draw statusbar
        {
            let content_left = format!("");
            let content_right = format!(" {} | {}", cursor.x, cursor.y);
            queue!(
                stdout,
                cursor::MoveTo(0, self.height.saturating_sub(2) as u16),
                style::Print(
                    format!(
                        "{:<padding$}{}",
                        content_left,
                        content_right,
                        padding = self.width - content_right.len() - 1
                    )
                    .white()
                    .on_grey()
                )
            )?;
        }

        // draw debug info on bottom
        queue!(
            stdout,
            cursor::MoveTo(0, self.height as u16 - 1),
            style::Print(self.debug_output.clone().dark_grey())
        )?;

        // draw line numbers
        for i in 0..(self.height.saturating_sub(2)) {
            queue!(stdout, cursor::MoveTo(0, i as u16))?;
            if self.offset.y + i < self.buffer.len() {
                let lineno = format!(
                    "{:>width$}",
                    i + self.offset.y + 1,
                    width = self.sidebar_width - 1
                );
                let num = if i + self.offset.y == cursor.y {
                    lineno.white()
                } else {
                    lineno.dark_grey()
                };
                write!(stdout, "{} {}", num, "│".dark_grey())?;
            } else {
                write!(
                    stdout,
                    "{}",
                    format!("{:>width$} {}", "~", "│", width = self.sidebar_width - 1).dark_grey()
                )?;
            }
        }

        let start = self.offset.y;
        let end = (self.offset.y + self.height)
            .saturating_sub(2)
            .min(self.buffer.len());

        for line_number in start..end {
            queue!(
                stdout,
                cursor::MoveTo(self.sidebar_width as u16 + 1, (line_number - start) as u16)
            )?;

            let view_end = self.offset.x + self.width - self.sidebar_width;
            let mut width = 0;
            for (g, w) in &self.buffer[line_number].rope {
                width += w;
                if width >= view_end {
                    break;
                }
                if self.offset.x < width {
                    write!(stdout, "{}", g)?;
                }
            }
        }

        execute!(
            stdout,
            cursor::MoveTo(
                (cursor.x - self.offset.x + self.sidebar_width + 1) as u16,
                (cursor.y - self.offset.y) as u16
            ),
            cursor::Show,
            terminal::EndSynchronizedUpdate
        )?;
        Ok(())
    }

    fn get_cursor_position(&self) -> Position {
        Position {
            x: self.buffer[self.cursor.y]
                .rope
                .iter()
                .take(self.cursor.x)
                .map(|g| g.1)
                .sum::<usize>(),
            y: self.cursor.y,
        }
    }

    fn update_sidebar_width(&mut self) {
        // Calculate sidebar width based on maximum possible line number
        let max_line_num = (self.offset.y + self.height)
            .saturating_sub(2)
            .min(self.buffer.len());
        self.sidebar_width = if max_line_num > 99 {
            (max_line_num as f64).log10().floor() as usize + 1
        } else {
            2
        } + 2; // the " │" part
    }

    fn update_offset(&mut self) {
        const EXTRA_GAP: usize = 3;

        let Position { x, y } = self.get_cursor_position();

        self.offset.y = self.offset.y.clamp(
            (y + EXTRA_GAP + 3).saturating_sub(self.height),
            y.saturating_sub(EXTRA_GAP),
        );

        self.offset.x = self.offset.x.clamp(
            (x + EXTRA_GAP + 1).saturating_sub(self.width - self.sidebar_width),
            x.saturating_sub(EXTRA_GAP),
        );
    }

    pub fn on_exit(&self) -> Result<(), Error> {
        let mut stdout = io::stdout();

        execute!(
            stdout,
            event::DisableFocusChange,
            event::DisableBracketedPaste,
            event::DisableMouseCapture,
            terminal::EnableLineWrap,
            terminal::LeaveAlternateScreen
        )?;
        terminal::disable_raw_mode()?;

        Ok(())
    }

    pub fn save(&self, filename: &str) -> Result<(), Error> {
        std::fs::write(
            filename,
            self.buffer
                .iter()
                .map(|row| row.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        )?;
        // TODO: Option to save with \r\n
        Ok(())
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        let _ = self.on_exit();
    }
}
