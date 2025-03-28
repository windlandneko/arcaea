use crossterm::event::{self, Event, KeyCode};

use crate::Error;

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
                    Event::Key(event) => match event.code {
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
                    Event::Key(event) => match event.code {
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

                    _ => {}
                }
            }
        }
    }
}
