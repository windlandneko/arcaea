use crossterm::{
    cursor, event, execute, queue,
    style::{self, ContentStyle, Print, StyledContent},
    terminal,
};
use std::io::{stdout, Stdout};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{editor::Position, Error};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pixel {
    /// The style (colors, content attributes).
    style: ContentStyle,
    /// A content to apply the style on.
    content: String,
}

impl Default for Pixel {
    fn default() -> Self {
        Pixel {
            style: ContentStyle::default(),
            content: " ".to_string(),
        }
    }
}

pub struct Terminal {
    stdout: Stdout,

    pub height: usize,
    pub width: usize,

    pub buffer: Vec<Vec<Pixel>>,
    last_buffer: Vec<Vec<Pixel>>,
}

impl Default for Terminal {
    fn default() -> Self {
        Terminal::new()
    }
}

impl Terminal {
    pub fn new() -> Self {
        let (width, height) = terminal::size().expect("Failed to get terminal size");
        Terminal {
            stdout: stdout(),
            height: height.into(),
            width: width.into(),

            buffer: vec![vec![Pixel::default(); width.into()]; height.into()],
            last_buffer: vec![vec![Pixel::default(); width.into()]; height.into()],
        }
    }

    pub fn init(&mut self) -> Result<(), Error> {
        terminal::enable_raw_mode()?;
        execute!(
            self.stdout,
            terminal::EnterAlternateScreen,
            terminal::DisableLineWrap,
            event::EnableMouseCapture,
            event::EnableBracketedPaste,
            event::EnableFocusChange,
        )?;
        Ok(())
    }

    pub fn cleanup(&mut self) -> Result<(), Error> {
        execute!(
            self.stdout,
            event::DisableFocusChange,
            event::DisableBracketedPaste,
            event::DisableMouseCapture,
            terminal::EnableLineWrap,
            terminal::LeaveAlternateScreen
        )?;
        terminal::disable_raw_mode()?;
        Ok(())
    }

    pub fn update_window_size(&mut self, height: u16, width: u16) {
        self.height = height as usize;
        self.width = width as usize;

        self.buffer = vec![vec![Pixel::default(); self.width]; self.height];
        self.last_buffer = vec![vec![Pixel::default(); self.width]; self.height];
    }

    pub fn begin_render(&mut self) -> Result<(), Error> {
        execute!(self.stdout, terminal::BeginSynchronizedUpdate, cursor::Hide)?;

        for row in &mut self.buffer {
            for pixel in row {
                *pixel = Pixel::default();
            }
        }

        Ok(())
    }

    pub fn end_render(&mut self) -> Result<(), Error> {
        let mut current_style = ContentStyle::default();
        queue!(self.stdout, cursor::SavePosition, style::ResetColor)?;

        let buffer_str = self
            .buffer
            .iter()
            .zip(self.last_buffer.iter())
            .map(|(current_row, last_row)| {
                current_row
                    .iter()
                    .zip(last_row.iter())
                    .map(|(pixel, last_pixel)| {
                        if pixel != last_pixel {
                            pixel.content.clone()
                        } else {
                            ".".to_string()
                        }
                    })
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n");
        std::fs::write("buffer.txt", buffer_str)
            .unwrap_or_else(|_| eprintln!("Failed to write buffer.txt"));

        for (y, row) in self.buffer.iter().enumerate() {
            let mut next_char = 0;
            let mut cursor_x = 0;
            queue!(self.stdout, cursor::MoveTo(0, y as u16))?;
            for (x, pixel) in row.iter().enumerate() {
                if x != next_char {
                    continue;
                }
                let last_pixel = &self.last_buffer[y][x];

                #[cfg(not(feature = "debug"))]
                {
                    if pixel != last_pixel {
                        if x != cursor_x {
                            queue!(self.stdout, cursor::MoveTo(x as u16, y as u16))?;
                            cursor_x = x;
                        }
                        if pixel.style != current_style {
                            queue!(self.stdout, style::SetStyle(pixel.style))?;
                            current_style = pixel.style;
                        }
                        queue!(self.stdout, Print(pixel.content.clone()))?;
                        cursor_x += pixel.content.width();
                    }
                    next_char += pixel.content.width();
                }

                #[cfg(feature = "debug")]
                {
                    if pixel != last_pixel {
                        let mut ch = ".";
                        if x != cursor_x {
                            queue!(self.stdout, cursor::MoveTo(x as u16, y as u16))?;
                            ch = ">";
                            cursor_x = x;
                        }
                        if pixel.style != current_style {
                            ch = if ch == ">" { "#" } else { "$" };
                            current_style = pixel.style;
                        }
                        queue!(self.stdout, Print(ch))?;
                        cursor_x += 1;
                    } else {
                        queue!(self.stdout, Print(" "))?;
                    }
                    next_char += 1;
                }
            }
        }

        execute!(
            self.stdout,
            cursor::RestorePosition,
            cursor::Show,
            terminal::EndSynchronizedUpdate
        )?;

        self.last_buffer = self.buffer.clone();

        Ok(())
    }

    pub fn write(&mut self, mut pos: Position, content: StyledContent<String>) {
        for ch in content.content().graphemes(true) {
            let width = ch.width();
            if pos.x + width >= self.width || pos.y >= self.height {
                break;
            }
            for i in 0..width {
                let pixel = &mut self.buffer[pos.y][pos.x + i];
                pixel.content = ch.to_string();
                pixel.style = *content.style();
            }
            pos.x += width;
        }
    }

    pub fn write_char(&mut self, pos: Position, content: StyledContent<&str>) {
        let ch = &content.content();
        let width = ch.width();
        if pos.x + width >= self.width || pos.y >= self.height {
            return;
        }
        for i in 0..width {
            let pixel = &mut self.buffer[pos.y][pos.x + i];
            pixel.content = ch.to_string();
            pixel.style = *content.style();
        }
    }
}
