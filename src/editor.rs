use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind},
    execute, queue,
    style::{self, Stylize},
    terminal,
};
use std::io::{self, Write};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{Error, Row, Tui};

const EXTRA_GAP: usize = 3;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Position {
    pub x: usize,
    pub y: usize,
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Position {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.y.cmp(&other.y) {
            std::cmp::Ordering::Equal => self.x.cmp(&other.x),
            ord => ord,
        }
    }
}

#[derive(Default)]
pub struct Editor {
    filename: Option<String>,

    buffer: Vec<Row>,
    status_string: String,
    height: usize,
    width: usize,

    sidebar_width: usize,

    viewbox: Position,
    cursor: Position,

    /// The position of the selection.
    /// None if not selected, Some if selected a range.
    anchor: Option<Position>,

    dirty: bool,
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

            self.dirty = true;
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
                let mut should_update_viewbox = true;

                match event::read()? {
                    // Keyboard Event
                    Event::Key(event) if event.kind != KeyEventKind::Release => {
                        match (event.modifiers, event.code) {
                            (KeyModifiers::CONTROL, KeyCode::Char('s')) => self.save_file()?,

                            (_, KeyCode::Esc)
                            | (KeyModifiers::CONTROL, KeyCode::Char('w' | 'W')) => {
                                match Tui::confirm_exit(self.dirty)? {
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

                            // Regular character input
                            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(char)) => {
                                self.dirty = true;

                                self.cursor.x = self.cursor.x.min(self.get_width());

                                if let Some((begin, end)) = self.get_selection() {
                                    self.delete_selection_range(begin, end);
                                }

                                self.buffer[self.cursor.y].rope.insert(
                                    self.cursor.x,
                                    (char.to_string(), char.width().unwrap_or(0)),
                                );
                                self.cursor.x += 1;
                            }

                            // Control character input
                            // viewbox or cursor's movement; delete; enter; etc.
                            (modifiers, code) => {
                                match code {
                                    KeyCode::Up => {
                                        if modifiers == KeyModifiers::ALT {
                                            let (begin, end) = self
                                                .get_selection()
                                                .unwrap_or_else(|| (self.cursor, self.cursor));
                                            if begin.y > 0 {
                                                for i in begin.y..=end.y {
                                                    self.buffer.swap(i - 1, i);
                                                }
                                                if let Some(anchor) = &mut self.anchor {
                                                    anchor.y -= 1;
                                                }
                                            }
                                        } else {
                                            self.update_selection(modifiers);
                                        }

                                        if modifiers == KeyModifiers::CONTROL {
                                            should_update_viewbox = false;

                                            self.viewbox.y = self.viewbox.y.saturating_sub(1);
                                        } else if self.cursor.y > 0 {
                                            self.cursor.y -= 1;
                                        } else {
                                            self.cursor.x = 0;
                                        }
                                    }
                                    KeyCode::Down => {
                                        if modifiers == KeyModifiers::ALT {
                                            let (begin, end) = self
                                                .get_selection()
                                                .unwrap_or_else(|| (self.cursor, self.cursor));
                                            if end.y < self.buffer.len() - 1 {
                                                for i in (begin.y..=end.y).rev() {
                                                    self.buffer.swap(i, i + 1);
                                                }
                                                if let Some(anchor) = &mut self.anchor {
                                                    anchor.y += 1;
                                                }
                                            }
                                        } else {
                                            self.update_selection(modifiers);
                                        }

                                        if modifiers == KeyModifiers::CONTROL {
                                            should_update_viewbox = false;

                                            self.viewbox.y = (self.viewbox.y + 1).min(
                                                (self.buffer.len() + EXTRA_GAP)
                                                    .saturating_sub(self.height - 2),
                                            );
                                        } else if self.cursor.y < self.buffer.len() - 1 {
                                            self.cursor.y += 1;
                                        } else {
                                            self.cursor.x = self.get_width();
                                        }
                                    }
                                    KeyCode::Left => {
                                        self.cursor.x = self.cursor.x.min(self.get_width());

                                        self.update_selection(modifiers);

                                        if modifiers == KeyModifiers::CONTROL {
                                            // Move to the beginning of the word
                                            if self.cursor.x == 0 && self.cursor.y > 0 {
                                                self.cursor.y -= 1;
                                                self.cursor.x = self.get_width();
                                            }
                                            while self.cursor.x > 0
                                                && self.buffer[self.cursor.y].rope
                                                    [self.cursor.x - 1]
                                                    .0
                                                    == " "
                                            {
                                                self.cursor.x -= 1;
                                            }
                                            while self.cursor.x > 0
                                                && self.buffer[self.cursor.y].rope
                                                    [self.cursor.x - 1]
                                                    .0
                                                    != " "
                                            {
                                                self.cursor.x -= 1;
                                            }
                                        } else if self.cursor.x > 0 {
                                            self.cursor.x -= 1;
                                        } else if self.cursor.y > 0 {
                                            self.cursor.y -= 1;
                                            self.cursor.x = self.get_width();
                                        }
                                    }
                                    KeyCode::Right => {
                                        self.cursor.x = self.cursor.x.min(self.get_width());
                                        self.update_selection(modifiers);

                                        if modifiers == KeyModifiers::CONTROL {
                                            // Move to the end of the word
                                            if self.cursor.x == self.get_width()
                                                && self.cursor.y < self.buffer.len() - 1
                                            {
                                                self.cursor.y += 1;
                                                self.cursor.x = 0;
                                            }
                                            while self.cursor.x
                                                < self.buffer[self.cursor.y].rope.len()
                                                && self.buffer[self.cursor.y].rope[self.cursor.x].0
                                                    == " "
                                            {
                                                self.cursor.x += 1;
                                            }
                                            while self.cursor.x
                                                < self.buffer[self.cursor.y].rope.len()
                                                && self.buffer[self.cursor.y].rope[self.cursor.x].0
                                                    != " "
                                            {
                                                self.cursor.x += 1;
                                            }
                                        } else if self.cursor.x < self.get_width() {
                                            self.cursor.x += 1;
                                        } else if self.cursor.y < self.buffer.len() - 1 {
                                            self.cursor.y += 1;
                                            self.cursor.x = 0;
                                        }
                                    }

                                    KeyCode::PageUp => {
                                        self.update_selection(modifiers);
                                        self.cursor.y =
                                            self.cursor.y.saturating_sub(self.height - 2);
                                    }
                                    KeyCode::PageDown => {
                                        self.update_selection(modifiers);
                                        self.cursor.y = (self.cursor.y + self.height - 2)
                                            .min(self.buffer.len() - 1);
                                    }
                                    KeyCode::Home => {
                                        self.update_selection(modifiers);
                                        self.cursor.x = 0;
                                    }
                                    KeyCode::End => {
                                        self.update_selection(modifiers);
                                        self.cursor.x = self.get_width();
                                    }

                                    KeyCode::Enter => {
                                        self.dirty = true;

                                        self.cursor.x = self.cursor.x.min(self.get_width());

                                        if let Some((begin, end)) = self.get_selection() {
                                            self.delete_selection_range(begin, end);
                                        }

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
                                        self.dirty = true;

                                        self.cursor.x = self.cursor.x.min(self.get_width());

                                        if let Some((begin, end)) = self.get_selection() {
                                            self.delete_selection_range(begin, end);
                                        } else if self.cursor.x > 0 {
                                            // The cursor is in the middle, just delete the char
                                            self.cursor.x -= 1;
                                            self.buffer[self.cursor.y].rope.remove(self.cursor.x);
                                        } else if self.cursor.y > 0 {
                                            // The cursor is in the beginning, and not at the first line
                                            // Merge the current line with the previous line
                                            self.cursor.y -= 1;
                                            self.cursor.x = self.get_width();
                                            let mut rope = self.buffer[self.cursor.y].rope.clone();
                                            rope.extend(self.buffer.remove(self.cursor.y + 1).rope);
                                            self.buffer[self.cursor.y] = Row { rope };
                                        }
                                    }
                                    KeyCode::Delete => {
                                        self.dirty = true;

                                        self.cursor.x = self.cursor.x.min(self.get_width());

                                        if let Some((begin, end)) = self.get_selection() {
                                            self.delete_selection_range(begin, end);
                                        } else if self.cursor.x < self.get_width() {
                                            // The cursor is in the middle, just delete the char
                                            self.buffer[self.cursor.y].rope.remove(self.cursor.x);
                                        } else if self.cursor.y < self.buffer.len() - 1 {
                                            // The cursor is in the end, and not at the last line
                                            // Merge the current line with the next line
                                            let mut rope = self.buffer[self.cursor.y].rope.clone();
                                            rope.extend(self.buffer.remove(self.cursor.y + 1).rope);
                                            self.buffer[self.cursor.y] = Row { rope };
                                        }
                                    }

                                    _ => {}
                                }
                            }
                        }
                    }

                    // Mouse Event
                    Event::Mouse(event) => match event.kind {
                        MouseEventKind::ScrollUp => {
                            should_update_viewbox = false;

                            let dt = if event.modifiers == KeyModifiers::ALT {
                                8
                            } else {
                                3
                            };
                            self.viewbox.y = self.viewbox.y.saturating_sub(dt);
                        }
                        MouseEventKind::ScrollDown => {
                            should_update_viewbox = false;

                            let dt = if event.modifiers == KeyModifiers::ALT {
                                8
                            } else {
                                3
                            };
                            self.viewbox.y = (self.viewbox.y + dt).min(
                                (self.buffer.len() + EXTRA_GAP).saturating_sub(self.height - 2),
                            );
                        }
                        MouseEventKind::ScrollLeft => {
                            should_update_viewbox = false;

                            self.viewbox.x = self.viewbox.x.saturating_sub(3);
                        }
                        MouseEventKind::ScrollRight => {
                            should_update_viewbox = false;

                            self.viewbox.x =
                                (self.viewbox.x + 3).min(self.get_width() + EXTRA_GAP + 1);
                        }

                        MouseEventKind::Down(MouseButton::Left)
                        | MouseEventKind::Drag(MouseButton::Left) => {
                            if (event.row as usize) < self.height - 2 {
                                self.cursor.y = event.row as usize + self.viewbox.y;
                                let x = (event.column as usize + self.viewbox.x)
                                    .saturating_sub(self.sidebar_width + 1);

                                if self.cursor.y >= self.buffer.len() {
                                    self.cursor.y = self.buffer.len() - 1;
                                    self.cursor.x = self.get_width();
                                } else if (event.column as usize) < self.sidebar_width {
                                    self.cursor.x = 0;
                                    self.cursor.y = event.row as usize + self.viewbox.y;
                                    if event.kind == MouseEventKind::Down(MouseButton::Left) {
                                        self.anchor = Some(self.cursor);
                                    }
                                    if self.cursor.y >= self.anchor.unwrap_or(self.cursor).y {
                                        self.cursor.y += 1;
                                        if self.cursor.y == self.buffer.len() {
                                            self.cursor.y = self.buffer.len() - 1;
                                            self.cursor.x = self.get_width();
                                        }
                                    }
                                } else {
                                    if x > self.get_width() {
                                        self.cursor.x = self.get_width();
                                    } else {
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

                                    if event.kind == MouseEventKind::Down(MouseButton::Left)
                                    // TODO: Make Shift+Drag work
                                    // && event.modifiers != KeyModifiers::SHIFT
                                    {
                                        self.anchor = Some(self.cursor);
                                    }
                                }
                            }
                        }

                        _ => {
                            should_update_viewbox = false;
                        }
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
                let status = format!(
                    " viewbox: ({}, {}) | cursor: ({}, {}) @ {:?} | view cursor: ({}, {})",
                    self.viewbox.y + 1,
                    self.viewbox.x + 1,
                    self.cursor.y + 1,
                    self.cursor.x + 1,
                    self.anchor.map(|a| (a.y + 1, a.x + 1)),
                    c.y + 1,
                    c.x + 1
                );
                self.status_string = format!("{:<width$}", status, width = self.width);

                if should_update_viewbox {
                    self.update_viewbox();
                }

                self.render()?;
            }
        }

        Ok(())
    }

    fn delete_selection_range(&mut self, begin: Position, end: Position) {
        // Range delete
        self.buffer[begin.y] = Row {
            rope: self.buffer[begin.y]
                .rope
                .iter()
                .take(begin.x)
                .chain(self.buffer[end.y].rope.iter().skip(end.x))
                .cloned()
                .collect(),
        };
        for index in (begin.y + 1..=end.y).rev() {
            self.buffer.remove(index);
        }
        // Reset cursor and anchor
        self.cursor = begin;
        self.anchor = None;
    }

    fn get_selection(&self) -> Option<(Position, Position)> {
        self.anchor.map(|anchor| {
            let cursor = self.cursor;
            if anchor < cursor {
                (anchor, cursor)
            } else {
                (cursor, anchor)
            }
        })
    }

    fn update_selection(&mut self, modifiers: KeyModifiers) {
        if modifiers == KeyModifiers::SHIFT {
            // if anchor is None, set it to cursor
            self.anchor.get_or_insert(self.cursor);
        } else {
            self.anchor = None;
        }
    }

    fn render(&mut self) -> Result<(), Error> {
        self.update_sidebar_width();

        let cursor = self.get_cursor_position();

        let mut stdout = io::stdout();

        execute!(
            stdout,
            terminal::BeginSynchronizedUpdate,
            cursor::Hide,
            // terminal::Clear(terminal::ClearType::All),
        )?;

        // draw statusbar
        {
            let content_left = format!(" {}", self.filename.as_deref().unwrap_or("Untitled"));
            let content_left = if self.dirty {
                format!("{} (未保存)", content_left)
            } else {
                content_left
            };
            let content_right = format!("行 {}，列 {} ", self.cursor.y + 1, self.cursor.x + 1);
            queue!(
                stdout,
                cursor::MoveTo(0, self.height.saturating_sub(2) as u16),
                style::Print(
                    format!(
                        "{}{}{}",
                        content_left,
                        " ".repeat(self.width - content_left.width() - content_right.width()),
                        content_right,
                    )
                    .with((219, 191, 239).into())
                    .on((40, 23, 51).into())
                )
            )?;
        }

        // draw debug info on bottom
        queue!(
            stdout,
            cursor::MoveTo(0, self.height as u16 - 1),
            style::Print(self.status_string.clone().dark_grey())
        )?;

        self.render_sidebar(cursor)?;

        let begin = self.viewbox.y;
        let end = (self.viewbox.y + self.height)
            .saturating_sub(2)
            .min(self.buffer.len());

        for line_number in begin..end {
            queue!(
                stdout,
                cursor::MoveTo(self.sidebar_width as u16 + 1, (line_number - begin) as u16)
            )?;

            let view_end = self.viewbox.x + self.width - self.sidebar_width;
            let mut width = 0;
            let mut last_color = None;
            for (i, (g, w)) in self.buffer[line_number].rope.iter().enumerate() {
                width += w;
                if width >= view_end {
                    break;
                }
                if self.viewbox.x < width {
                    let fg_color = (255, 255, 255);

                    let mut bg_color = (59, 34, 76);
                    if let Some(range) = self.get_selection() {
                        let current = Position {
                            y: line_number,
                            x: i,
                        };
                        if range.0 <= current && current < range.1 {
                            bg_color = (164, 160, 232);
                        }
                    }
                    let current_color = Some((fg_color, bg_color));
                    if last_color != current_color {
                        queue!(
                            stdout,
                            style::SetForegroundColor(fg_color.into()),
                            style::SetBackgroundColor(bg_color.into())
                        )?;
                    }
                    last_color = current_color;
                    write!(stdout, "{}", g)?;
                }
            }

            queue!(
                stdout,
                style::SetBackgroundColor((59, 34, 76).into()),
                terminal::Clear(terminal::ClearType::UntilNewLine)
            )?;
        }

        self.render_cursor(cursor)?;

        execute!(stdout, terminal::EndSynchronizedUpdate)?;
        Ok(())
    }

    fn render_sidebar(&self, cursor: Position) -> Result<(), Error> {
        let mut stdout = io::stdout();
        queue!(stdout, style::SetBackgroundColor((59, 34, 76).into()))?;
        for i in 0..(self.height.saturating_sub(2)) {
            queue!(stdout, cursor::MoveTo(0, i as u16))?;
            if self.viewbox.y + i < self.buffer.len() {
                let lineno = format!(
                    "{:>width$} ",
                    i + self.viewbox.y + 1,
                    width = self.sidebar_width
                );
                let num = if i + self.viewbox.y == cursor.y {
                    lineno.with((219, 191, 239).into())
                } else {
                    lineno.with((90, 89, 119).into())
                };
                write!(stdout, "{}", num)?;
            } else {
                queue!(stdout, terminal::Clear(terminal::ClearType::UntilNewLine),)?;
                write!(
                    stdout,
                    "{} ",
                    format!("{:>width$}", "~", width = self.sidebar_width)
                        .with((90, 89, 119).into())
                )?;
            }
        }

        Ok(())
    }

    fn render_cursor(&self, cursor: Position) -> Result<(), Error> {
        let mut stdout = io::stdout();
        let (x, y) = (
            cursor.x as isize - self.viewbox.x as isize + self.sidebar_width as isize + 1,
            cursor.y as isize - self.viewbox.y as isize,
        );

        if x >= 0 && x < self.width as isize && y >= 0 && y < self.height as isize {
            queue!(stdout, cursor::MoveTo(x as u16, y as u16), cursor::Show)?;
        }

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
        let max_line_num = (self.viewbox.y + self.height)
            .saturating_sub(2)
            .min(self.buffer.len());
        self.sidebar_width = if max_line_num > 99 {
            (max_line_num as f64).log10().floor() as usize + 1
        } else {
            2
        } + 1; // the " │" part
    }

    fn update_viewbox(&mut self) {
        let Position { x, y } = self.get_cursor_position();

        self.viewbox.y = self.viewbox.y.clamp(
            (y + EXTRA_GAP + 3).saturating_sub(self.height),
            y.saturating_sub(EXTRA_GAP),
        );

        self.viewbox.x = self.viewbox.x.clamp(
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

        self.dirty = false;
        Ok(())
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        let _ = self.on_exit();
    }
}
