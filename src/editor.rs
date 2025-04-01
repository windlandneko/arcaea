use crossterm::{
    cursor,
    event::{
        self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    queue,
    style::Stylize,
};
use std::io::{self, Write};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{style, Error, History, Row, Terminal, Tui};

const EXTRA_GAP: usize = 2;

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

    history: History<Row>,
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
                .map(Row::from)
                .collect();
        } else {
            self.buffer = Vec::new();
            self.buffer.push(Row::from(""));

            self.dirty = true;
        }

        self.history
            .push_state(&self.buffer, self.viewbox, self.cursor, self.anchor);

        self.terminal.init()?;

        if self.check_minimum_window_size() {
            self.render()?;
        }

        self.event_loop()?;

        self.on_exit()?;

        Ok(())
    }

    fn event_loop(&mut self) -> Result<(), Error> {
        let mut cnt = 0;
        let mut mouse: Option<MouseEvent> = None;
        loop {
            let mut should_update_viewbox = true;
            if event::poll(std::time::Duration::from_millis(25))? {
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

                            // Select ALL
                            (KeyModifiers::CONTROL, KeyCode::Char('a' | 'A')) => {
                                self.anchor = Some(Position { x: 0, y: 0 });
                                self.cursor.y = self.buffer.len() - 1;
                                self.cursor.x = self.get_width();
                                should_update_viewbox = false;
                            }

                            // Undo
                            (KeyModifiers::CONTROL, KeyCode::Char('z' | 'Z')) => {
                                if self.history.undo() {
                                    self.buffer = self.history.current.clone();
                                    self.viewbox = self.history.current_state.viewbox;
                                    self.cursor = self.history.current_state.cursor;
                                    self.anchor = self.history.current_state.anchor;
                                }
                            }

                            // Redo
                            (KeyModifiers::CONTROL, KeyCode::Char('y' | 'Y')) => {
                                if self.history.redo() {
                                    self.buffer = self.history.current.clone();
                                    self.viewbox = self.history.current_state.viewbox;
                                    self.cursor = self.history.current_state.cursor;
                                    self.anchor = self.history.current_state.anchor;
                                }
                            }

                            // Regular character input
                            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(char)) => {
                                self.update_last_history_state();
                                self.dirty = true;

                                self.cursor.x = self.cursor.x.min(self.get_width());

                                if let Some((begin, end)) = self.get_selection() {
                                    self.delete_selection_range(begin, end);
                                }

                                self.buffer[self.cursor.y].0.insert(
                                    self.cursor.x,
                                    (char.to_string(), char.width().unwrap_or(0)),
                                );
                                self.cursor.x += 1;

                                self.create_history();
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
                                                self.update_last_history_state();
                                                self.dirty = true;

                                                for i in begin.y..=end.y {
                                                    self.buffer.swap(i - 1, i);
                                                }
                                                if let Some(anchor) = &mut self.anchor {
                                                    anchor.y -= 1;
                                                }

                                                self.create_history();
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
                                                self.update_last_history_state();
                                                self.dirty = true;

                                                for i in (begin.y..=end.y).rev() {
                                                    self.buffer.swap(i, i + 1);
                                                }
                                                if let Some(anchor) = &mut self.anchor {
                                                    anchor.y += 1;
                                                }

                                                self.create_history();
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
                                                && self.buffer[self.cursor.y].0[self.cursor.x - 1].0
                                                    == " "
                                            {
                                                self.cursor.x -= 1;
                                            }
                                            while self.cursor.x > 0
                                                && self.buffer[self.cursor.y].0[self.cursor.x - 1].0
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
                                            while self.cursor.x < self.buffer[self.cursor.y].0.len()
                                                && self.buffer[self.cursor.y].0[self.cursor.x].0
                                                    == " "
                                            {
                                                self.cursor.x += 1;
                                            }
                                            while self.cursor.x < self.buffer[self.cursor.y].0.len()
                                                && self.buffer[self.cursor.y].0[self.cursor.x].0
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
                                        self.update_last_history_state();
                                        self.dirty = true;

                                        self.cursor.x = self.cursor.x.min(self.get_width());

                                        if let Some((begin, end)) = self.get_selection() {
                                            self.delete_selection_range(begin, end);
                                        }

                                        let new_line = Row(self.buffer[self.cursor.y].0
                                            [self.cursor.x..]
                                            .to_vec());
                                        self.buffer.insert(self.cursor.y + 1, new_line);
                                        self.buffer[self.cursor.y] =
                                            Row(self.buffer[self.cursor.y].0[..self.cursor.x]
                                                .to_vec());
                                        self.cursor.y += 1;
                                        self.cursor.x = 0;

                                        self.create_history();
                                    }

                                    KeyCode::Backspace => {
                                        self.update_last_history_state();
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
                                            self.buffer[self.cursor.y].0.remove(self.cursor.x);
                                        } else if self.cursor.y > 0 {
                                            // The cursor is in the beginning, and not at the first line
                                            // Merge the current line with the previous line
                                            self.cursor.y -= 1;
                                            self.cursor.x = self.get_width();
                                            let mut row = self.buffer[self.cursor.y].0.clone();
                                            row.extend(self.buffer.remove(self.cursor.y + 1).0);
                                            self.buffer[self.cursor.y] = Row(row);
                                        }

                                        self.create_history();
                                    }
                                    KeyCode::Delete => {
                                        self.update_last_history_state();
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
                                            self.buffer[self.cursor.y].0.remove(self.cursor.x);
                                        } else if self.cursor.y < self.buffer.len() - 1 {
                                            // The cursor is in the end, and not at the last line
                                            // Merge the current line with the next line
                                            let mut row = self.buffer[self.cursor.y].0.clone();
                                            row.extend(self.buffer.remove(self.cursor.y + 1).0);
                                            self.buffer[self.cursor.y] = Row(row);
                                        }

                                        self.create_history();
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
                            mouse = Some(event);
                        }

                        MouseEventKind::Up(MouseButton::Left) => {
                            mouse = None;
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
            }

            if let Some(event) = mouse {
                if (event.row as usize) < self.terminal.height - 2 {
                    self.cursor.y = event.row as usize + self.viewbox.y;
                    let x =
                        (event.column as usize + self.viewbox.x).saturating_sub(self.sidebar_width);

                    if self.cursor.y >= self.buffer.len() {
                        self.cursor.y = self.buffer.len() - 1;
                        self.cursor.x = self.get_width();
                    } else if (event.column as usize) < self.sidebar_width {
                        self.cursor.x = 0;
                        self.cursor.y = event.row as usize + self.viewbox.y;
                        if event.kind == MouseEventKind::Down(MouseButton::Left) {
                            // self.anchor = Some(self.cursor);
                            should_update_viewbox = false;
                        }
                        if self.cursor.y >= self.anchor.unwrap_or(self.cursor).y {
                            self.cursor.y += 1;
                            if self.cursor.y == self.buffer.len() {
                                self.cursor.y = self.buffer.len() - 1;
                                self.cursor.x = self.get_width();
                            }
                        }
                    } else {
                        let visual_width = self.buffer[self.cursor.y]
                            .0
                            .iter()
                            .map(|g| g.1)
                            .sum::<usize>();
                        if x > visual_width {
                            self.cursor.x = self.get_width();
                        } else {
                            let mut width = 0;
                            for (i, cell) in self.buffer[self.cursor.y].0.iter().enumerate() {
                                if width + cell.1 / 2 >= x {
                                    self.cursor.x = i;
                                    break;
                                }
                                width += cell.1;
                            }
                        }
                    }

                    // TODO: Make Shift+Drag work
                    // && event.modifiers != KeyModifiers::SHIFT
                    if event.kind == MouseEventKind::Down(MouseButton::Left) {
                        self.anchor = Some(self.cursor);
                    }
                }
            }

            let c = self.get_cursor_position();
            self.status_string = format!(
                " viewbox: ({}, {}) | cursor: ({}, {}) @ {:?} | view cursor: ({}, {}) | Frame = {}",
                self.viewbox.y + 1,
                self.viewbox.x + 1,
                self.cursor.y + 1,
                self.cursor.x + 1,
                self.anchor.map(|a| (a.y + 1, a.x + 1)),
                c.y + 1,
                c.x + 1,
                cnt,
            );
            cnt += 1;

            if !self.check_minimum_window_size() {
                continue;
            }

            if should_update_viewbox {
                self.update_viewbox();
            }

            self.render()?;
        }

        Ok(())
    }

    fn delete_selection_range(&mut self, begin: Position, end: Position) {
        // Range delete
        self.buffer[begin.y] = Row(self.buffer[begin.y]
            .0
            .iter()
            .take(begin.x)
            .chain(self.buffer[end.y].0.iter().skip(end.x))
            .cloned()
            .collect());
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
                " ".repeat(self.terminal.width).on(style::background),
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
                .with(style::text_primary)
                .on(style::background_primary),
            );
        }

        // draw debug info on bottom
        self.terminal.write(
            (0, self.terminal.height - 1).into(),
            self.status_string
                .clone()
                .with(style::text_linenum)
                .on(style::background),
        );

        self.render_sidebar(cursor);

        let begin = self.viewbox.y;
        let end = (self.viewbox.y + self.terminal.height - 2).min(self.buffer.len());

        for line_number in begin..end {
            let mut dx = self.sidebar_width as isize - self.viewbox.x as isize;
            for (i, (g, w)) in self.buffer[line_number]
                .0
                .iter()
                .chain([&("\n".to_string(), 1)]) // Append a virtual space to the end of the line
                .enumerate()
            {
                dx += *w as isize;
                if dx >= self.terminal.width as isize {
                    break;
                }
                if dx >= (self.sidebar_width + w) as isize {
                    let mut str = g.as_str();
                    let fg_color = style::text;
                    let mut bg_color = style::background;

                    if let Some((begin, end)) = self.get_selection() {
                        let current = (i, line_number).into();
                        if begin <= current && current < end {
                            bg_color = style::background_selected;
                            // if str == " " {
                            //     str = "•";
                            //     g_color = style::text_selected_whitespace;
                            // }
                        }
                    }
                    if str == "\n" {
                        str = " ";
                    }
                    self.terminal.write_char(
                        (dx as usize - w, line_number - self.viewbox.y).into(),
                        str.with(fg_color).on(bg_color),
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
                    lineno.with(style::text_linenum_selected)
                } else {
                    lineno.with(style::text_linenum)
                };
                self.terminal
                    .write((0, i).into(), num.on(style::background));
            } else {
                self.terminal.write(
                    (0, i).into(),
                    format!("{:>width$} ", "~", width = self.sidebar_width - 1)
                        .with(style::text_linenum),
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
                .0
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

    fn create_history(&mut self) {
        self.history
            .push_state(&self.buffer, self.viewbox, self.cursor, self.anchor);
    }
    fn update_last_history_state(&mut self) {
        self.history
            .update_state(self.viewbox, self.cursor, self.anchor);
    }

    fn on_exit(&mut self) -> Result<(), Error> {
        self.terminal.cleanup()?;

        Ok(())
    }

    fn save_file(&mut self) -> Result<(), Error> {
        self.update_last_history_state();

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
                .map(|line| line.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        )?;
        // TODO: Option to save with \r\n

        self.dirty = false;

        self.create_history();
        Ok(())
    }
}
