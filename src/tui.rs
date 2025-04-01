use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    style::Stylize,
};
use unicode_width::UnicodeWidthChar;

use crate::{editor::Position, style, Error, Row, Terminal};

#[derive(Default)]
pub struct Input {
    viewbox: Position,

    offset: usize,
    cursor: usize,
    max_width: usize,

    pub input: Row,
}

impl Input {
    pub fn new(viewbox: Position, max_width: usize) -> Self {
        Self {
            viewbox,
            offset: 0,
            cursor: 0,
            max_width,
            input: Row::default(),
        }
    }

    pub fn event_loop(&mut self) -> Result<(), Error> {
        loop {
            if event::poll(std::time::Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(event) if event.kind != KeyEventKind::Release => match event.code {
                        KeyCode::Char(c) => {
                            self.input
                                .rope
                                .push((c.to_string(), c.width().unwrap_or(0)));
                        }
                        KeyCode::Esc => {
                            return Ok(());
                        }
                        KeyCode::Enter => {
                            return Ok(());
                        }

                        KeyCode::Backspace => {
                            self.input.rope.pop();
                        }
                        _ => {}
                    },

                    Event::Mouse(_) => {
                        todo!("Mouse event handling");
                    }

                    _ => {}
                }
            }
        }
    }

    pub fn render(&self, terminal: &mut Terminal) -> Result<(), Error> {
        terminal.write(
            self.viewbox,
            " ".repeat(self.max_width).on(style::background),
        );

        Ok(())
    }
}

pub struct Tui {}

impl Tui {
    pub fn confirm_exit(changed: bool) -> Result<Option<bool>, Error> {
        if !changed {
            return Ok(Some(true));
        }

        println!("[退出程序] 是否保存? (Y/n)");

        loop {
            if event::poll(std::time::Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(event) if event.kind != KeyEventKind::Release => match event.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                            return Ok(Some(true));
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') => {
                            return Ok(Some(false));
                        }
                        KeyCode::Esc => {
                            return Ok(None);
                        }

                        _ => {}
                    },

                    Event::Mouse(_) => {
                        todo!("Mouse event handling");
                    }

                    _ => {}
                }
            }
        }
    }

    pub fn prompt_filename() -> Result<Option<String>, Error> {
        println!("输入文件名: ");

        let mut filename = String::new();

        loop {
            if event::poll(std::time::Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(event) if event.kind != KeyEventKind::Release => match event.code {
                        KeyCode::Char(c) => {
                            filename.push(c);
                        }
                        KeyCode::Esc => {
                            return Ok(None);
                        }
                        KeyCode::Enter => {
                            return Ok(Some(filename));
                        }

                        _ => {}
                    },

                    Event::Mouse(_) => {
                        todo!("Mouse event handling");
                    }

                    _ => {}
                }
            }
        }
    }
}
