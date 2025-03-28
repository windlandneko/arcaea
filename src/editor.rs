use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind},
    execute, queue,
    style::{self, Stylize},
    terminal,
};
use std::io::{self, Write};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{Error, Row, Tui};

const EXTRA_GAP: usize = 3;

#[derive(Debug, Clone, Copy, Default)]
pub struct Position {
    pub x: usize,
    pub y: usize,
}

#[derive(Default)]
pub struct Editor {
    filename: Option<String>,

    buffer: Vec<Row>,
    debug_output: String,
    height: usize,
    width: usize,

    sidebar_width: usize,

    offset: Position,
    cursor: Position,

    changed: bool,
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

        self.filename = filename.clone();

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
            terminal::EnterAlternateScreen,
            terminal::DisableLineWrap,
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
                    Event::Key(event) => match (event.modifiers, event.code) {
                        (KeyModifiers::CONTROL, KeyCode::Char('s')) => self.save_file()?,
                        (_, KeyCode::Esc) | (KeyModifiers::CONTROL, KeyCode::Char('w' | 'W')) => {
                            match Tui::confirm_exit(self.changed)? {
                                Some(true) => {
                                    self.save_file()?;
                                    break;
                                }
                                Some(false) => {
                                    break;
                                }
                                None => {}
                            }
                        }
                        (KeyModifiers::NONE, code) => {
                            match code {
                                KeyCode::Up => {
                                    if self.cursor.y > 0 {
                                        self.cursor.y -= 1;
                                    } else {
                                        self.cursor.x = 0;
                                    }
                                }
                                KeyCode::Down => {
                                    if self.cursor.y < self.buffer.len() - 1 {
                                        self.cursor.y += 1;
                                    } else {
                                        self.cursor.x = self.get_width();
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
                                    self.cursor.y = (self.cursor.y + self.height - 2)
                                        .min(self.buffer.len() - 1);
                                }
                                KeyCode::Home => {
                                    self.cursor.x = 0;
                                }
                                KeyCode::End => {
                                    self.cursor.x = self.get_width();
                                }

                                KeyCode::Enter => {
                                    self.changed = true;

                                    let new_line = Row {
                                        rope: self.buffer[self.cursor.y].rope[self.cursor.x..]
                                            .to_vec(),
                                    };
                                    self.buffer.insert(self.cursor.y + 1, new_line);
                                    self.buffer[self.cursor.y] = Row {
                                        rope: self.buffer[self.cursor.y].rope[..self.cursor.x]
                                            .to_vec(),
                                    };
                                    self.cursor.y += 1;
                                    self.cursor.x = 0;
                                }

                                KeyCode::Backspace => {
                                    self.changed = true;

                                    if self.cursor.x > 0 {
                                        self.cursor.x -= 1;
                                        self.buffer[self.cursor.y].rope.remove(self.cursor.x);
                                    } else if self.cursor.y > 0 {
                                        self.cursor.y -= 1;
                                        self.cursor.x = self.get_width();
                                        let mut rope = self.buffer[self.cursor.y].rope.clone();
                                        rope.extend(self.buffer.remove(self.cursor.y + 1).rope);
                                        self.buffer[self.cursor.y] = Row { rope };
                                    }
                                }
                                KeyCode::Delete => {
                                    self.changed = true;

                                    if self.cursor.x < self.get_width() {
                                        self.buffer[self.cursor.y].rope.remove(self.cursor.x);
                                    } else if self.cursor.y < self.buffer.len() - 1 {
                                        let mut rope = self.buffer[self.cursor.y].rope.clone();
                                        rope.extend(self.buffer.remove(self.cursor.y + 1).rope);
                                        self.buffer[self.cursor.y] = Row { rope };
                                    }
                                }

                                KeyCode::Char(c) => {
                                    self.changed = true;

                                    self.buffer[self.cursor.y].rope.insert(
                                        self.cursor.x,
                                        (c.to_string(), c.width().unwrap_or(0)),
                                    );
                                    self.cursor.x += 1;
                                }

                                _ => {}
                            }
                            self.update_offset();
                        }

                        _ => {}
                    },
                    Event::Mouse(event) => match event.kind {
                        MouseEventKind::ScrollUp => {
                            self.offset.y = self.offset.y.saturating_sub(3);
                        }
                        MouseEventKind::ScrollDown => {
                            self.offset.y = (self.offset.y + 3).min(
                                (self.buffer.len() + EXTRA_GAP).saturating_sub(self.height - 2),
                            );
                        }
                        MouseEventKind::ScrollLeft => {
                            self.offset.x = self.offset.x.saturating_sub(3);
                        }
                        MouseEventKind::ScrollRight => {
                            self.offset.x =
                                (self.offset.x + 3).min(self.get_width() + EXTRA_GAP + 1);
                        }

                        MouseEventKind::Down(btn) => match btn {
                            MouseButton::Left => {
                                if event.row < self.height as u16 - 2 {
                                    self.cursor.y = event.row as usize + self.offset.y;
                                    let x =
                                        (event.column as usize).saturating_sub(self.sidebar_width);

                                    let mut width = 0;
                                    for (i, cell) in
                                        self.buffer[self.cursor.y].rope.iter().enumerate()
                                    {
                                        if width + cell.1 / 2 >= x {
                                            self.cursor.x = i;
                                            break;
                                        }
                                        width += cell.1;
                                    }
                                }
                            }

                            _ => {}
                        },

                        _ => {}
                    },

                    Event::Resize(width, height) => {
                        (self.height, self.width) = (height as usize, width as usize);
                    }
                    _ => {}
                }

                if self.width < 5 || self.height < 5 {
                    continue;
                }

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
            let content_left = format!(" {}", self.filename.as_deref().unwrap_or("Untitled"));
            let content_left = if self.changed {
                format!("{} (未保存)", content_left)
            } else {
                content_left
            };
            let content_right = format!("行 {}，列 {} ", cursor.x, cursor.y);
            queue!(
                stdout,
                cursor::MoveTo(0, self.height.saturating_sub(2) as u16),
                style::Print(
                    format!(
                        "{}{}{}",
                        content_left,
                        " ".repeat(self.width - content_left.width() - content_right.width() - 1),
                        content_right,
                    )
                    .reverse()
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

        let (x, y) = (
            cursor.x as isize - self.offset.x as isize + self.sidebar_width as isize + 1,
            cursor.y as isize - self.offset.y as isize,
        );

        if x >= 0 && x < self.width as isize && y >= 0 && y < self.height as isize {
            execute!(stdout, cursor::MoveTo(x as u16, y as u16), cursor::Show)?;
        }
        execute!(stdout, terminal::EndSynchronizedUpdate)?;
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

    pub fn save_file(&mut self) -> Result<(), Error> {
        if self.filename.is_none() {
            self.filename = Tui::prompt_filename()?;
        }

        if self.filename.is_none() {
            return Ok(());
        }

        std::fs::write(
            self.filename.clone().unwrap(),
            self.buffer
                .iter()
                .map(|row| row.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        )?;
        // TODO: Option to save with \r\n

        self.changed = false;
        Ok(())
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        let _ = self.on_exit();
    }
}
