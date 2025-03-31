use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind},
    queue,
    style::Stylize,
};
use std::io::{self, Write};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{Error, Row, Terminal, Tui};

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

impl From<(usize, usize)> for Position {
    fn from(value: (usize, usize)) -> Self {
        Position {
            x: value.0,
            y: value.1,
        }
    }
}

#[derive(Default)]
pub struct Editor {
    filename: Option<String>,

    buffer: Vec<Row>,
    status_string: String,
    terminal: Terminal,

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

        self.terminal.init()?;

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
                                    // TODO: Move cursor by visual offset, not logical offset
                                    KeyCode::Up => {
                                        if modifiers == KeyModifiers::ALT {
                                            let (begin, end) = self
                                                .get_selection()
                                                .unwrap_or((self.cursor, self.cursor));
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
                                                .unwrap_or((self.cursor, self.cursor));
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
                                                    .saturating_sub(self.terminal.height - 2),
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
                                            self.cursor.y.saturating_sub(self.terminal.height - 2);
                                    }
                                    KeyCode::PageDown => {
                                        self.update_selection(modifiers);
                                        self.cursor.y = (self.cursor.y + self.terminal.height - 2)
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

                                        // Fix wrong deletion when selection is empty
                                        if let Some((begin, end)) = self.get_selection() {
                                            if begin == end {
                                                self.anchor = None;
                                            }
                                        }

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

                                        // Fix wrong deletion when selection is empty
                                        if let Some((begin, end)) = self.get_selection() {
                                            if begin == end {
                                                self.anchor = None;
                                            }
                                        }

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
                                3
                            } else {
                                1
                            };
                            self.viewbox.y = self.viewbox.y.saturating_sub(dt);
                        }
                        MouseEventKind::ScrollDown => {
                            should_update_viewbox = false;

                            let dt = if event.modifiers == KeyModifiers::ALT {
                                3
                            } else {
                                1
                            };
                            self.viewbox.y = (self.viewbox.y + dt).min(
                                (self.buffer.len() + EXTRA_GAP)
                                    .saturating_sub(self.terminal.height - 2),
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
                            if (event.row as usize) < self.terminal.height - 2 {
                                self.cursor.y = event.row as usize + self.viewbox.y;
                                let x = (event.column as usize + self.viewbox.x)
                                    .saturating_sub(self.sidebar_width);

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
                        self.terminal.update_window_size(height, width);
                    }
                    _ => {}
                }

                let c = self.get_cursor_position();
                self.status_string = format!(
                    " viewbox: ({}, {}) | cursor: ({}, {}) @ {:?} | view cursor: ({}, {})",
                    self.viewbox.y + 1,
                    self.viewbox.x + 1,
                    self.cursor.y + 1,
                    self.cursor.x + 1,
                    self.anchor.map(|a| (a.y + 1, a.x + 1)),
                    c.y + 1,
                    c.x + 1
                );

                if !self.check_minimum_window_size() {
                    continue;
                }

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
        self.terminal.begin_render()?;

        self.update_sidebar_width();

        let cursor = self.get_cursor_position();

        for i in 0..self.terminal.height {
            self.terminal.write(
                (0, i).into(),
                " ".repeat(self.terminal.width).on((59, 34, 76).into()),
            );
        }

        // draw statusbar
        {
            let content_left = format!(" {}", self.filename.as_deref().unwrap_or("Untitled"));
            let content_left = if self.dirty {
                format!("{} (未保存)", content_left)
            } else {
                content_left
            };
            let content_right = format!("行 {}，列 {} ", self.cursor.y + 1, self.cursor.x + 1);
            self.terminal.write(
                (0, self.terminal.height.saturating_sub(2)).into(),
                format!(
                    "{}{}{}",
                    content_left,
                    " ".repeat(self.terminal.width - content_left.width() - content_right.width()),
                    content_right,
                )
                .with((219, 191, 239).into())
                .on((40, 23, 51).into()),
            );
        }

        // draw debug info on bottom
        self.terminal.write(
            (0, self.terminal.height - 1).into(),
            self.status_string
                .clone()
                .with((90, 89, 119).into())
                .on((59, 34, 76).into()),
        );

        self.render_sidebar(cursor);

        let begin = self.viewbox.y;
        let end = (self.viewbox.y + self.terminal.height - 2).min(self.buffer.len());

        for line_number in begin..end {
            let mut dx = self.sidebar_width as isize - self.viewbox.x as isize;
            for (i, (g, w)) in self.buffer[line_number]
                .rope
                .iter()
                .chain([&(" ".to_string(), 1)]) // Append a virtual space to the end of the line
                .enumerate()
            {
                dx += *w as isize;
                if dx >= self.terminal.width as isize {
                    break;
                }
                if dx >= (self.sidebar_width + w) as isize {
                    let fg_color = (255, 255, 255);
                    let mut bg_color = (59, 34, 76);

                    if let Some((begin, end)) = self.get_selection() {
                        let current = (i, line_number).into();
                        if begin <= current && current < end {
                            bg_color = (164, 160, 232);
                        }
                    }
                    self.terminal.write_char(
                        (dx as usize - w, line_number - self.viewbox.y).into(),
                        g.as_str().with(fg_color.into()).on(bg_color.into()),
                    );
                }
            }
        }

        self.render_cursor(cursor)?;

        self.terminal.end_render()?;

        Ok(())
    }

    fn check_minimum_window_size(&mut self) -> bool {
        const MIN_WIDTH: usize = 40;
        const MIN_HEIGHT: usize = 9;
        if self.terminal.width < MIN_WIDTH || self.terminal.height < MIN_HEIGHT {
            let mut stdout = io::stdout();

            let _ = queue!(
                stdout,
                crossterm::cursor::Hide,
                crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
            );
            let (w, h) = (self.terminal.width, self.terminal.height);
            let (w_str, h_str) = (format!("{}", w), format!("{}", h));

            let hint_0 = "窗口过小";
            let _ = queue!(
                stdout,
                cursor::MoveTo(((w - hint_0.width()) / 2) as u16, (h / 2 - 1) as u16),
                crossterm::style::Print(hint_0.bold()),
            );
            let hint_1 = format!("Width = {}, Height = {}", w, h);
            let _ = queue!(
                stdout,
                cursor::MoveTo(((w - hint_1.width()) / 2) as u16, (h / 2) as u16),
                crossterm::style::Print(format!(
                    "Width = {}, Height = {}",
                    if w < MIN_WIDTH {
                        w_str.red().bold().slow_blink()
                    } else {
                        w_str.green().bold()
                    },
                    if h < MIN_HEIGHT {
                        h_str.red().bold().rapid_blink()
                    } else {
                        h_str.green().bold()
                    }
                ),),
            );
            let hint_2 = format!("(min width = {}, height = {})   ", MIN_WIDTH, MIN_HEIGHT);
            let _ = queue!(
                stdout,
                cursor::MoveTo(((w - hint_2.width()) / 2) as u16, (h / 2 + 1) as u16),
                crossterm::style::Print(hint_2),
            );
            let _ = stdout.flush();

            false
        } else {
            true
        }
    }

    fn render_sidebar(&mut self, cursor: Position) {
        for i in 0..(self.terminal.height.saturating_sub(2)) {
            if self.viewbox.y + i < self.buffer.len() {
                let lineno = format!(
                    "{:>width$} ",
                    i + self.viewbox.y + 1,
                    width = self.sidebar_width - 1
                );
                let num = if i + self.viewbox.y == cursor.y {
                    lineno.with((219, 191, 239).into())
                } else {
                    lineno.with((90, 89, 119).into())
                };
                self.terminal
                    .write((0, i).into(), num.on((59, 34, 76).into()));
            } else {
                self.terminal.write(
                    (0, i).into(),
                    format!("{:>width$} ", "~", width = self.sidebar_width - 1)
                        .with((90, 89, 119).into()),
                );
            }
        }
    }

    fn render_cursor(&self, cursor: Position) -> Result<(), Error> {
        let mut stdout = io::stdout();
        let (x, y) = (
            cursor.x as isize - self.viewbox.x as isize + self.sidebar_width as isize,
            cursor.y as isize - self.viewbox.y as isize,
        );

        if x >= 0 && x < self.terminal.width as isize && y >= 0 && y < self.terminal.height as isize
        {
            queue!(stdout, cursor::MoveTo(x as u16, y as u16))?;
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
        let max_line_num = (self.viewbox.y + self.terminal.height)
            .saturating_sub(2)
            .min(self.buffer.len());
        self.sidebar_width = if max_line_num > 99 {
            (max_line_num as f64).log10().floor() as usize + 1
        } else {
            2
        } + 2;
    }

    fn update_viewbox(&mut self) {
        let Position { x, y } = self.get_cursor_position();

        self.viewbox.y = self.viewbox.y.clamp(
            (y + EXTRA_GAP + 3).saturating_sub(self.terminal.height),
            y.saturating_sub(EXTRA_GAP),
        );

        self.viewbox.x = self.viewbox.x.clamp(
            (x + EXTRA_GAP + 1).saturating_sub(self.terminal.width - self.sidebar_width),
            x.saturating_sub(EXTRA_GAP),
        );
    }

    pub fn on_exit(&mut self) -> Result<(), Error> {
        self.terminal.cleanup()?;

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
