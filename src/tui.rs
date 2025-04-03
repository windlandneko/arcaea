use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind},
    style::{Color, Stylize},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{editor::Position, style, Editor, Error, Row, Terminal};

#[derive(Default)]
pub struct Input {
    pub viewbox: Position,

    offset: usize,
    cursor: usize,
    pub max_width: usize,

    pub buffer: Row,

    dragging: bool,
}

impl Input {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_event(&mut self, event: &Event) -> Result<Option<bool>, Error> {
        match event {
            Event::Key(event) if event.kind != KeyEventKind::Release => match event.code {
                KeyCode::Esc => {
                    return Ok(Some(false));
                }
                KeyCode::Enter => {
                    return Ok(Some(true));
                }

                KeyCode::Left => {
                    if event.modifiers.contains(KeyModifiers::CONTROL) {
                        // Move to the beginning of the word
                        while self.cursor > 0 && self.buffer.rope[self.cursor - 1].0 == " " {
                            self.cursor -= 1;
                        }
                        while self.cursor > 0 && self.buffer.rope[self.cursor - 1].0 != " " {
                            self.cursor -= 1;
                        }
                    } else if self.cursor > 0 {
                        self.cursor -= 1;
                    }
                }
                KeyCode::Right => {
                    if event.modifiers.contains(KeyModifiers::CONTROL) {
                        while self.cursor < self.buffer.len()
                            && self.buffer.rope[self.cursor].0 == " "
                        {
                            self.cursor += 1;
                        }
                        while self.cursor < self.buffer.len()
                            && self.buffer.rope[self.cursor].0 != " "
                        {
                            self.cursor += 1;
                        }
                    } else if self.cursor < self.buffer.len() {
                        self.cursor += 1;
                    }
                }
                KeyCode::Home => {
                    self.cursor = 0;
                }
                KeyCode::End => {
                    self.cursor = self.buffer.len();
                }

                KeyCode::Char(char) => {
                    self.cursor = self.cursor.min(self.buffer.len());

                    self.buffer
                        .rope
                        .insert(self.cursor, (char.to_string(), char.width().unwrap_or(0)));
                    self.cursor += 1;
                }

                KeyCode::Backspace => {
                    if self.cursor > 0 {
                        self.cursor -= 1;
                        self.buffer.rope.remove(self.cursor);
                    }
                }
                KeyCode::Delete => {
                    if self.cursor < self.buffer.len() {
                        self.buffer.rope.remove(self.cursor);
                    }
                }
                _ => {}
            },

            Event::Mouse(event) => match event.kind {
                MouseEventKind::Down(MouseButton::Left)
                | MouseEventKind::Drag(MouseButton::Left) => {
                    let (x, y) = (event.column as usize, event.row as usize);

                    if self.dragging
                        || (y == self.viewbox.y
                            && x >= self.viewbox.x
                            && x < self.viewbox.x + self.max_width)
                    {
                        if matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
                            self.dragging = true;
                        }

                        let x = (x + self.offset).saturating_sub(self.viewbox.x);
                        let visual_width = self.buffer.rope.iter().map(|g| g.1).sum::<usize>();
                        if x > visual_width {
                            self.cursor = self.buffer.len();
                        } else {
                            let mut width = 0;
                            for (i, cell) in self.buffer.rope.iter().enumerate() {
                                if width >= x {
                                    self.cursor = i;
                                    break;
                                }
                                width += cell.1;
                            }
                        }
                    }
                }

                _ => {
                    self.dragging = false;
                }
            },

            _ => {}
        }

        self.offset = self.offset.clamp(
            (self.cursor + 1).saturating_sub(self.max_width),
            self.cursor,
        );

        Ok(None)
    }

    pub fn render(&self, term: &mut Terminal) {
        term.write(
            self.viewbox,
            " ".repeat(self.max_width)
                .with(style::text_model)
                .on(style::background)
                .underlined(),
        );

        let mut dx = -(self.offset as isize);
        for (g, w) in self.buffer.rope.iter() {
            dx += *w as isize;
            if dx >= self.max_width as isize {
                break;
            }
            if dx >= *w as isize {
                term.write_char(
                    (dx as usize + self.viewbox.x - 1, self.viewbox.y).into(),
                    g.as_str()
                        .with(style::text_model)
                        .on(style::background)
                        .underlined(),
                );
            }
        }

        let visual_width: usize = self.buffer.rope.iter().take(self.cursor).map(|g| g.1).sum();
        term.cursor = Some(
            (
                (self.viewbox.x + visual_width).saturating_sub(self.offset),
                self.viewbox.y,
            )
                .into(),
        );
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
        let text = self.text.to_string().with(self.color).on(style::background);
        if !self.hover {
            term.write((x + 2, y + 1).into(), text);
        } else {
            term.write((x + 2, y + 1).into(), text.bold());
            if let Some(ref hint) = self.hint {
                term.write(
                    (x + 2, y + 2).into(),
                    hint.to_string().with(self.color).on(style::background),
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
            editor.render_to_buffer();
            self.render(&mut editor.terminal)?;
        }

        loop {
            if event::poll(std::time::Duration::from_millis(25))? {
                match event::read()? {
                    Event::Key(event) if event.kind != KeyEventKind::Release => match event.code {
                        KeyCode::Char('y' | 'Y') | KeyCode::Enter => {
                            return Ok(Some(true));
                        }
                        KeyCode::Char('n' | 'N') => {
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

                        if let MouseEventKind::Down(_) = event.kind {
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
                .to_string()
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

struct Prompt {
    title: String,
    input: Input,
    yes: Button,
    no: Button,
}

impl Prompt {
    pub fn new(title: String, yes: String, no: String) -> Self {
        let yes = Button::new(yes, style::text_model_primary, Some("Yes".to_string()));
        let no = Button::new(no, style::text_model, Some("No".to_string()));
        let mut input = Input::new();
        input.max_width = 256;
        Self {
            title,
            input,
            yes,
            no,
        }
    }

    pub fn event_loop(&mut self, editor: &mut Editor) -> Result<Option<String>, Error> {
        if editor.check_minimum_window_size() {
            editor.render_to_buffer();
            self.render(&mut editor.terminal)?;
        }

        loop {
            if event::poll(std::time::Duration::from_millis(25))? {
                let event = event::read()?;
                match self.input.handle_event(&event)? {
                    Some(true) => {
                        if !self.input.buffer.is_empty() {
                            return Ok(Some(self.input.buffer.to_string()));
                        }
                    }
                    Some(false) => {
                        return Ok(None);
                    }
                    None => {
                        if let Event::Mouse(event) = event {
                            self.yes.hover = false;
                            self.no.hover = false;

                            let mouse = (event.column as usize, event.row as usize);

                            let (w, h) =
                                ((self.title.width() + 16).min(editor.terminal.width - 5), 8);
                            let (x, y) = (
                                (editor.terminal.width - w) / 2,
                                (editor.terminal.height - 2 - h) / 2,
                            );

                            let buttons_offset = self.yes.width + self.no.width + 10;
                            let mut offset = (x + w - buttons_offset, y + h - 2);
                            if !self.input.buffer.is_empty() {
                                self.yes.intersect(offset, mouse);
                            }
                            offset.0 += self.yes.width + 5;
                            self.no.intersect(offset, mouse);

                            if let MouseEventKind::Down(_) = event.kind {
                                if self.yes.hover {
                                    return Ok(Some(self.input.buffer.to_string()));
                                } else if self.no.hover {
                                    return Ok(None);
                                }
                            }
                        }
                    }
                }
            }

            if !editor.check_minimum_window_size() {
                continue;
            }

            editor.render_to_buffer();
            self.render(&mut editor.terminal)?;
        }
    }

    pub fn render(&mut self, term: &mut Terminal) -> Result<(), Error> {
        term.dimmed()?;

        let (w, h) = ((self.title.width() + 16).min(term.width - 5), 8);
        let (x, y) = ((term.width - w) / 2, (term.height - 2 - h) / 2);

        term.begin_render()?;

        draw_rounded_rect(term, (x, y), (w, h), style::text_model, style::background);

        term.write(
            (x + 3, y).into(),
            " PROMPT "
                .to_string()
                .bold()
                .with(style::text_primary)
                .on(style::text_model),
        );
        term.write(
            (x + 3, y + 2).into(),
            self.title
                .to_string()
                .with(style::text_model)
                .on(style::background),
        );

        self.input.viewbox = (x + 3, y + 4).into();
        self.input.max_width = w - 4;
        self.input.render(term);

        let buttons_offset = self.yes.width + self.no.width + 10;
        let mut offset = (x + w - buttons_offset, y + h - 2);
        self.yes.render(term, offset)?;
        offset.0 += self.yes.width + 5;
        self.no.render(term, offset)?;

        term.end_render()?;

        Ok(())
    }
}

struct Alert {
    title: String,
    message: String,
    yes: Button,
}

impl Alert {
    pub fn new(title: String, message: String, yes: String) -> Self {
        let yes = Button::new(yes, style::text_model_primary, Some("Fuck".to_string()));
        Self {
            title,
            message,
            yes,
        }
    }

    pub fn event_loop(&mut self, editor: &mut Editor) -> Result<(), Error> {
        if editor.check_minimum_window_size() {
            editor.render_to_buffer();
            self.render(&mut editor.terminal)?;
        }

        loop {
            if event::poll(std::time::Duration::from_millis(25))? {
                match event::read()? {
                    Event::Key(event) if event.kind != KeyEventKind::Release => match event.code {
                        KeyCode::Char('y' | 'Y') | KeyCode::Enter | KeyCode::Esc => {
                            return Ok(());
                        }

                        _ => {}
                    },

                    Event::Mouse(event) => {
                        self.yes.hover = false;

                        let mouse = (event.column as usize, event.row as usize);

                        let title_width = self.title.width();
                        let message_width = self.message.width();

                        let (w, h) = (
                            (message_width.max(title_width) + 12).min(editor.terminal.width - 5),
                            8,
                        );
                        let (x, y) = (
                            (editor.terminal.width - w) / 2,
                            (editor.terminal.height - 2 - h) / 2,
                        );

                        self.yes
                            .intersect((x + (w - self.yes.width) / 2 - 2, y + h - 2), mouse);

                        if let MouseEventKind::Down(_) = event.kind {
                            if self.yes.hover {
                                return Ok(());
                            }
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
        let message_width = self.message.width();

        let (w, h) = ((message_width.max(title_width) + 12).min(term.width - 5), 8);
        let (x, y) = ((term.width - w) / 2, (term.height - 2 - h) / 2);

        term.begin_render()?;

        draw_rounded_rect(term, (x, y), (w, h), style::text_model, style::background);

        term.write(
            (x + w / 2 - 3, y).into(),
            " ALERT "
                .to_string()
                .bold()
                .with(style::text_primary)
                .on(style::text_model),
        );
        term.write(
            (x + (w - title_width) / 2 + 1, y + 2).into(),
            self.title
                .to_string()
                .bold()
                .with(style::text_alert)
                .on(style::background),
        );
        term.write(
            (x + (w - message_width) / 2 + 1, y + 4).into(),
            self.message
                .to_string()
                .with(style::text_model)
                .on(style::background),
        );

        self.yes
            .render(term, (x + (w - self.yes.width) / 2 - 1, y + h - 2))?;

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
            format!(
                "是否要保存对 {} 的更改？",
                editor.filename.clone().unwrap_or("Untitled".to_string())
            ),
            "保存".to_string(),
            "不保存".to_string(),
            Some("取消".to_string()),
        )
        .event_loop(editor)
    }

    pub fn prompt_filename(editor: &mut Editor) -> Result<Option<String>, Error> {
        Prompt::new(
            "请输入文件名: ".to_string(),
            "保存".to_string(),
            "取消".to_string(),
        )
        .event_loop(editor)
    }

    pub fn confirm_overwrite(
        editor: &mut Editor,
        filename: &String,
    ) -> Result<Option<bool>, Error> {
        Confirm::new(
            format!("文件 {} 已存在，是否覆盖？", filename),
            "覆盖".to_string(),
            "取消".to_string(),
            None,
        )
        .event_loop(editor)
    }

    pub fn alert(editor: &mut Editor, title: String, message: String) -> Result<(), Error> {
        Alert::new(title, message, "好吧".to_string()).event_loop(editor)
    }
}
