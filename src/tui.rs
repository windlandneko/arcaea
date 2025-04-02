use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, MouseEventKind},
    style::{Color, Stylize},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{editor::Position, style, Editor, Error, Row, Terminal};

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
                            self.input.0.push((c.to_string(), c.width().unwrap_or(0)));
                        }
                        KeyCode::Esc => {
                            return Ok(());
                        }
                        KeyCode::Enter => {
                            return Ok(());
                        }

                        KeyCode::Backspace => {
                            self.input.0.pop();
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

    pub fn render(&self, term: &mut Terminal) -> Result<(), Error> {
        term.write(
            self.viewbox,
            " ".repeat(self.max_width).on(style::background),
        );

        Ok(())
    }
}

fn draw_rounded_rect(
    t: &mut Terminal,
    (x, y): (usize, usize),
    (w, h): (usize, usize),
    text_color: Color,
    background_color: Color,
) {
    macro_rules! colored {
        ($str:expr) => {
            $str.with(text_color).on(background_color)
        };
    }

    t.write_char((x, y).into(), colored!("╭"));
    t.write_char((x + w + 1, y).into(), colored!("╮"));
    t.write_char((x, y + h + 1).into(), colored!("╰"));
    t.write_char((x + w + 1, y + h + 1).into(), colored!("╯"));

    for i in 1..=h {
        t.write_char((x, y + i).into(), colored!("│"));
        t.write_char((x + w + 1, y + i).into(), colored!("│"));
        t.write((x + 1, y + i).into(), colored!(" ".repeat(w)));
    }
    for i in 1..=w {
        t.write_char((x + i, y).into(), colored!("─"));
        t.write_char((x + i, y + h + 1).into(), colored!("─"));
    }
}

struct Button {
    text: String,
    color: Color,
    width: usize,
    hint: Option<String>,

    pub hover: bool,
}

impl Button {
    pub fn new(text: String, color: Color, hint: Option<String>) -> Self {
        let width = text.width();
        Self {
            text,
            width,
            color,
            hint,
            hover: false,
        }
    }

    pub fn render(&self, term: &mut Terminal, (x, y): (usize, usize)) -> Result<(), Error> {
        draw_rounded_rect(
            term,
            (x, y),
            (self.width + 2, 1),
            self.color,
            style::background,
        );
        let text = self.text.clone().with(self.color).on(style::background);
        if self.hover {
            term.write((x + 2, y + 1).into(), text.underlined());
        } else {
            term.write((x + 2, y + 1).into(), text);
            if let Some(ref hint) = self.hint {
                term.write(
                    (x + 2, y + 2).into(),
                    hint.clone().with(self.color).on(style::background),
                );
            }
        }

        Ok(())
    }

    pub fn intersect(&mut self, offset: (usize, usize), mouse: (usize, usize)) {
        self.hover = (offset.0 <= mouse.0 && mouse.0 < offset.0 + self.width + 4)
            && (offset.1 <= mouse.1 && mouse.1 < offset.1 + 3);
    }
}

pub struct Confirm {
    title: String,
    yes: Button,
    no: Button,
    cancel: Option<Button>,
}

impl Confirm {
    pub fn new(title: String, yes: String, no: String, cancel: Option<String>) -> Self {
        let yes = Button::new(yes, style::text_model_primary, Some("Yes".to_string()));
        let no = Button::new(no, style::text_model, Some("No".to_string()));
        let cancel = cancel.map(|s| Button::new(s, style::text_model, Some("Esc".to_string())));

        Self {
            title,
            yes,
            no,
            cancel,
        }
    }

    pub fn event_loop(&mut self, editor: &mut Editor) -> Result<Option<bool>, Error> {
        if editor.check_minimum_window_size() {
            self.render(&mut editor.terminal)?;
        }

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

                    Event::Mouse(event) => {
                        self.yes.hover = false;
                        self.no.hover = false;
                        if let Some(ref mut button) = self.cancel {
                            button.hover = false;
                        }

                        let mouse = (event.column as usize, event.row as usize);

                        let title_width = self.title.width();
                        let cancel_width = self.cancel.as_ref().map_or(0, |s| s.width + 5);
                        let buttons_offset = self.yes.width + 5 + self.no.width + 5 + cancel_width;

                        let (w, h) = (
                            (title_width.max(buttons_offset) + 16).min(editor.terminal.width - 5),
                            6,
                        );
                        let (x, y) = (
                            (editor.terminal.width - w) / 2,
                            (editor.terminal.height - 2 - h) / 2,
                        );

                        let mut offset = (x + w - buttons_offset, y + h - 2);

                        self.yes.intersect(offset, mouse);
                        offset.0 += self.yes.width + 5;
                        self.no.intersect(offset, mouse);
                        offset.0 += self.no.width + 5;
                        if let Some(ref mut cancel_button) = self.cancel {
                            cancel_button.intersect(offset, mouse);
                        }

                        match event.kind {
                            MouseEventKind::Down(_) => {
                                if self.yes.hover {
                                    return Ok(Some(true));
                                } else if self.no.hover {
                                    return Ok(Some(false));
                                } else if let Some(ref cancel_button) = self.cancel {
                                    if cancel_button.hover {
                                        return Ok(None);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }

                    Event::Resize(width, height) => {
                        editor.terminal.update_window_size(height, width);
                    }

                    _ => {}
                }

                if !editor.check_minimum_window_size() {
                    continue;
                }

                editor.render_to_buffer();
                self.render(&mut editor.terminal)?;
            }
        }
    }

    pub fn render(&self, term: &mut Terminal) -> Result<(), Error> {
        term.dimmed()?;

        let title_width = self.title.width();
        let cancel_width = self.cancel.as_ref().map_or(0, |s| s.width + 5);
        let buttons_offset = self.yes.width + 5 + self.no.width + 5 + cancel_width;

        let (w, h) = (
            (title_width.max(buttons_offset) + 16).min(term.width - 5),
            6,
        );
        let (x, y) = ((term.width - w) / 2, (term.height - 2 - h) / 2);

        term.begin_render()?;

        draw_rounded_rect(term, (x, y), (w, h), style::text_model, style::background);

        term.write(
            (x + 3, y).into(),
            " CONFIRM "
                .to_string()
                .bold()
                .with(style::text_primary)
                .on(style::text_model),
        );
        term.write(
            (x + 3, y + 2).into(),
            self.title
                .clone()
                .with(style::text_model)
                .on(style::background),
        );

        let mut offset = (x + w - buttons_offset, y + h - 2);
        self.yes.render(term, offset)?;
        offset.0 += self.yes.width + 5;
        self.no.render(term, offset)?;
        if let Some(ref cancel_button) = self.cancel {
            offset.0 += self.no.width + 5;
            cancel_button.render(term, offset)?;
        }

        term.end_render()?;

        Ok(())
    }
}

pub struct Tui {}

impl Tui {
    pub fn confirm_exit(editor: &mut Editor) -> Result<Option<bool>, Error> {
        if !editor.dirty {
            return Ok(Some(false));
        }

        Confirm::new(
            "是否要保存对文件的更改？".to_string(),
            "保存".to_string(),
            "不保存".to_string(),
            Some("取消".to_string()),
        )
        .event_loop(editor)
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
