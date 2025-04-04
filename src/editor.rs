use crossterm::{
    cursor,
    event::{
        self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    queue,
    style::Stylize,
};
use std::{
    io::{self, Write},
    path::Path,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{
    style,
    syntax::{TokenState, TokenType},
    tui::Input,
    Error, History, Row, Syntax, Terminal, Tui,
};

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

impl<T: Into<usize>> From<(T, T)> for Position {
    fn from(value: (T, T)) -> Self {
        Position {
            x: value.0.into(),
            y: value.1.into(),
        }
    }
}

#[derive(Default)]
pub struct Editor {
    pub filename: Option<String>,
    is_crlf: bool,

    buffer: Vec<Row>,
    status_string: String,
    pub terminal: Terminal,

    sidebar_width: usize,

    viewbox: Position,
    cursor: Position,

    /// The position of the selection.
    /// None if not selected, Some if selected a range.
    anchor: Option<Position>,

    pub dirty: bool,

    history: History<Row>,
    syntax: Syntax,

    search: Input,
    search_result: Vec<Position>,
    is_searching: bool,
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
                .map(|line| {
                    if line.ends_with('\r') {
                        self.is_crlf = true;
                    }
                    line.strip_suffix('\r').unwrap_or(line)
                })
                .map(Row::from)
                .collect();

            let ext = Path::new(&name)
                .extension()
                .and_then(std::ffi::OsStr::to_str);
            if let Some(s) = ext.and_then(|e| Syntax::get(e).transpose()) {
                self.syntax = s?;
                self.update_syntax();
            } else {
                self.syntax = Syntax::default();
            }
        } else {
            self.buffer = Vec::new();
            self.buffer.push(Row::from(""));
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
        let mut dragging_sidebar = false;
        loop {
            let mut should_update_viewbox = true;
            if event::poll(std::time::Duration::from_millis(25))? {
                match event::read()? {
                    // Keyboard Event
                    Event::Key(event) if event.kind != KeyEventKind::Release => {
                        match (event.modifiers, event.code) {
                            (KeyModifiers::CONTROL, KeyCode::Char('s' | 'S'))
                            | (KeyModifiers::SHIFT, KeyCode::F(12)) => {
                                self.try_save_file(event.code == KeyCode::F(12))?;
                            }

                            (_, KeyCode::Esc)
                            | (KeyModifiers::CONTROL, KeyCode::Char('w' | 'W')) => {
                                match Tui::confirm_exit(self)? {
                                    Some(true) => {
                                        if self.try_save_file(false)? {
                                            break;
                                        }
                                    }
                                    Some(false) => {
                                        break;
                                    }
                                    None => {}
                                };
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

                                    // TODO: set dirty flag by really checking if the buffer is changed
                                    self.dirty = true;
                                }
                            }

                            // Redo
                            (KeyModifiers::CONTROL, KeyCode::Char('y' | 'Y')) => {
                                if self.history.redo() {
                                    self.buffer = self.history.current.clone();
                                    self.viewbox = self.history.current_state.viewbox;
                                    self.cursor = self.history.current_state.cursor;
                                    self.anchor = self.history.current_state.anchor;

                                    // TODO: set dirty flag by really checking if the buffer is changed
                                    self.dirty = true;
                                }
                            }

                            // Copy & Cut
                            (KeyModifiers::CONTROL, KeyCode::Char('c' | 'C' | 'x' | 'X')) => {
                                self.trigger_copy()?;

                                if event.code == KeyCode::Char('x')
                                    || event.code == KeyCode::Char('X')
                                {
                                    self.update_last_history_state();
                                    self.dirty = true;
                                    if let Some((begin, end)) = self.get_selection() {
                                        self.delete_selection_range(begin, end);
                                    } else {
                                        // Just delete the current line
                                        self.delete_selection_range(
                                            (0, self.cursor.y).into(),
                                            (self.get_width(), self.cursor.y).into(),
                                        );
                                    }
                                    self.create_history();
                                }
                            }

                            // Paste
                            (KeyModifiers::CONTROL, KeyCode::Char('v' | 'V')) => {
                                self.trigger_paste();
                            }

                            // Search
                            (KeyModifiers::CONTROL, KeyCode::Char('f' | 'F')) => {
                                self.into_search_mode()?;
                            }

                            // Regular character input
                            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(char)) => {
                                self.update_last_history_state();
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

                                self.create_history();
                            }

                            (_, KeyCode::Tab) => {
                                self.update_last_history_state();
                                self.dirty = true;

                                self.cursor.x = self.cursor.x.min(self.get_width());

                                if let Some((begin, end)) = self.get_selection() {
                                    self.delete_selection_range(begin, end);
                                }

                                self.buffer[self.cursor.y]
                                    .rope
                                    .insert(self.cursor.x, ("    ".to_string(), 4));
                                self.cursor.x += 1;

                                self.create_history();
                            }

                            // Control character input
                            // viewbox or cursor's movement; delete; enter; etc.
                            (modifiers, code) => {
                                match code {
                                    // TODO: Move cursor by visual offset, not logical offset
                                    KeyCode::Up => {
                                        if modifiers == KeyModifiers::ALT | KeyModifiers::SHIFT {
                                            let (begin, end) = self
                                                .get_selection()
                                                .unwrap_or((self.cursor, self.cursor));
                                            self.update_last_history_state();
                                            self.dirty = true;

                                            for i in (begin.y..=end.y).rev() {
                                                self.buffer
                                                    .insert(end.y + 1, self.buffer[i].clone());
                                            }

                                            self.create_history();
                                        } else {
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

                                            if modifiers.contains(KeyModifiers::CONTROL) {
                                                should_update_viewbox = false;

                                                self.viewbox.y = self.viewbox.y.saturating_sub(1);
                                            } else if self.cursor.y > 0 {
                                                self.cursor.y -= 1;
                                            } else {
                                                self.cursor.x = 0;
                                            }
                                        }
                                    }
                                    KeyCode::Down => {
                                        if modifiers == KeyModifiers::ALT | KeyModifiers::SHIFT {
                                            let (begin, end) = self
                                                .get_selection()
                                                .unwrap_or((self.cursor, self.cursor));
                                            self.update_last_history_state();
                                            self.dirty = true;

                                            for i in (begin.y..=end.y).rev() {
                                                self.buffer
                                                    .insert(end.y + 1, self.buffer[i].clone());
                                            }

                                            self.cursor.y += end.y - begin.y + 1;
                                            if let Some(anchor) = &mut self.anchor {
                                                anchor.y += end.y - begin.y + 1;
                                            }

                                            self.create_history();
                                        } else {
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

                                            if modifiers.contains(KeyModifiers::CONTROL) {
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
                                    }
                                    KeyCode::Left => {
                                        self.cursor.x = self.cursor.x.min(self.get_width());

                                        // Fix wrong deletion when selection is empty
                                        if let Some((begin, end)) = self.get_selection() {
                                            if begin == end {
                                                self.anchor = None;
                                            }
                                        }

                                        let mut flag = false;
                                        if let Some((begin, _)) = self.get_selection() {
                                            self.cursor.x = begin.x;
                                            flag = true;
                                        }
                                        self.update_selection(modifiers);

                                        if modifiers.contains(KeyModifiers::CONTROL) {
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
                                        } else if !flag && self.cursor.x > 0 {
                                            self.cursor.x -= 1;
                                        } else if !flag && self.cursor.y > 0 {
                                            self.cursor.y -= 1;
                                            self.cursor.x = self.get_width();
                                        }
                                    }
                                    KeyCode::Right => {
                                        self.cursor.x = self.cursor.x.min(self.get_width());

                                        // Fix wrong deletion when selection is empty
                                        if let Some((begin, end)) = self.get_selection() {
                                            if begin == end {
                                                self.anchor = None;
                                            }
                                        }

                                        let mut flag = false;
                                        if let Some((_, end)) = self.get_selection() {
                                            self.cursor.x = end.x;
                                            flag = true;
                                        }
                                        self.update_selection(modifiers);

                                        if modifiers.contains(KeyModifiers::CONTROL) {
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
                                        } else if !flag && self.cursor.x < self.get_width() {
                                            self.cursor.x += 1;
                                        } else if !flag && self.cursor.y < self.buffer.len() - 1 {
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

                                        let new_line = Row::from(
                                            self.buffer[self.cursor.y].rope[self.cursor.x..]
                                                .to_vec(),
                                        );
                                        self.buffer.insert(self.cursor.y + 1, new_line);
                                        self.buffer[self.cursor.y] = Row::from(
                                            self.buffer[self.cursor.y].rope[..self.cursor.x]
                                                .to_vec(),
                                        );
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
                                            self.buffer[self.cursor.y].rope.remove(self.cursor.x);
                                        } else if self.cursor.y > 0 {
                                            // The cursor is in the beginning, and not at the first line
                                            // Merge the current line with the previous line
                                            self.cursor.y -= 1;
                                            self.cursor.x = self.get_width();
                                            let mut row = self.buffer[self.cursor.y].rope.clone();
                                            row.extend(self.buffer.remove(self.cursor.y + 1).rope);
                                            self.buffer[self.cursor.y] = Row::from(row);
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
                                            self.buffer[self.cursor.y].rope.remove(self.cursor.x);
                                        } else if self.cursor.y < self.buffer.len() - 1 {
                                            // The cursor is in the end, and not at the last line
                                            // Merge the current line with the next line
                                            let mut row = self.buffer[self.cursor.y].rope.clone();
                                            row.extend(self.buffer.remove(self.cursor.y + 1).rope);
                                            self.buffer[self.cursor.y] = Row::from(row);
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

                            let dt = if event.modifiers.contains(KeyModifiers::ALT) {
                                5
                            } else {
                                2
                            };
                            self.viewbox.y = self.viewbox.y.saturating_sub(dt);
                        }
                        MouseEventKind::ScrollDown => {
                            should_update_viewbox = false;

                            let dt = if event.modifiers.contains(KeyModifiers::ALT) {
                                5
                            } else {
                                2
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
                            dragging_sidebar = false;
                        }

                        MouseEventKind::Down(MouseButton::Right) => {
                            // Fix wrong deletion when selection is empty
                            if let Some((begin, end)) = self.get_selection() {
                                if begin == end {
                                    self.anchor = None;
                                }
                            }

                            if let Some((_, end)) = self.get_selection() {
                                self.trigger_copy()?;
                                self.cursor = end;
                                self.anchor = None;
                            } else {
                                self.trigger_paste();
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
            } else if mouse.is_none() {
                continue;
            } else {
                #[cfg(feature = "debug")]
                continue;
            }

            if let Some(event) = mouse {
                if !(event.kind == MouseEventKind::Down(MouseButton::Left)
                    && (event.row as usize) >= self.terminal.height - 2)
                    || dragging_sidebar
                {
                    self.cursor.y = event.row as usize + self.viewbox.y;
                    let x =
                        (event.column as usize + self.viewbox.x).saturating_sub(self.sidebar_width);

                    if self.cursor.y >= self.buffer.len() {
                        self.cursor.y = self.buffer.len() - 1;
                        self.cursor.x = self.get_width();
                    }
                    if (event.column as usize) < self.sidebar_width {
                        self.cursor.x = 0;
                        self.cursor.y = event.row as usize + self.viewbox.y;
                        if event.kind == MouseEventKind::Down(MouseButton::Left) {
                            self.anchor = Some(self.cursor);
                            dragging_sidebar = true;
                            should_update_viewbox = false;
                        }
                        if dragging_sidebar && self.cursor.y >= self.anchor.unwrap_or(self.cursor).y
                        {
                            self.cursor.y += 1;
                            if self.cursor.y >= self.buffer.len() {
                                self.cursor.y = self.buffer.len() - 1;
                                self.cursor.x = self.get_width();
                            }
                        }
                    } else {
                        if event.column + 1 >= self.terminal.width as u16 {
                            self.cursor.x = self.get_width();
                        } else {
                            let visual_width = self.buffer[self.cursor.y]
                                .rope
                                .iter()
                                .map(|g| g.1)
                                .sum::<usize>();
                            if x >= visual_width {
                                self.cursor.x = self.get_width();
                            } else {
                                let mut width = 0;
                                for (i, cell) in self.buffer[self.cursor.y].rope.iter().enumerate()
                                {
                                    if width >= x {
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
        self.buffer[begin.y] = Row::from(
            self.buffer[begin.y]
                .rope
                .iter()
                .take(begin.x)
                .chain(self.buffer[end.y].rope.iter().skip(end.x))
                .cloned()
                .collect::<Vec<_>>(),
        );
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
        if modifiers.contains(KeyModifiers::SHIFT) {
            // if anchor is None, set it to cursor
            self.anchor.get_or_insert(self.cursor);
        } else {
            self.anchor = None;
        }
    }

    fn render(&mut self) -> Result<(), Error> {
        self.terminal.clear_buffer();
        self.terminal.begin_render()?;

        self.render_to_buffer();
        self.render_cursor();

        self.terminal.end_render()?;

        Ok(())
    }

    pub fn render_to_buffer(&mut self) {
        self.update_sidebar_width();

        for i in 0..self.terminal.height {
            self.terminal.write(
                (0, i).into(),
                " ".repeat(self.terminal.width).on(style::background),
            );
        }

        // draw statusbar
        {
            const LOGO_WIDTH: usize = 8;
            self.terminal.write(
                (0, self.terminal.height - 2).into(),
                " ARCAEA "
                    .to_string()
                    .with(style::text_primary)
                    .on(style::background_primary),
            );
            let content_left = format!(" {}", self.filename.as_deref().unwrap_or("Untitled"));
            let content_left = if self.dirty {
                format!("{} (未保存)", content_left)
            } else {
                content_left
            };
            let content_right = format!(
                "行 {}，列 {}  {} {} ",
                self.cursor.y + 1,
                self.cursor.x + 1,
                if self.is_crlf { "CRLF " } else { "LF " },
                self.syntax.name,
            );
            self.terminal.write(
                (LOGO_WIDTH, self.terminal.height.saturating_sub(2)).into(),
                format!(
                    "{}{}{}",
                    content_left,
                    " ".repeat(
                        self.terminal.width.saturating_sub(
                            content_left.width() + content_right.width() + LOGO_WIDTH
                        )
                    ),
                    content_right,
                )
                .with(style::text_statusbar)
                .on(style::background_sidebar),
            );
        }

        // draw debug info on bottom
        self.terminal.write(
            (0, self.terminal.height - 1).into(),
            self.status_string
                .clone()
                .with(style::text_dimmed)
                .on(style::background),
        );

        if self.is_searching {
            self.render_search();
        }

        self.render_sidebar();

        let begin = self.viewbox.y;
        let end = (self.viewbox.y + self.terminal.height - 2).min(self.buffer.len());

        for line_number in begin..end {
            let mut dx = self.sidebar_width as isize - self.viewbox.x as isize;
            for (i, (g, w)) in self.buffer[line_number]
                .rope
                .iter()
                .chain([(&("\n".to_string(), 1))]) // Append a virtual space to the end of the line
                .enumerate()
            {
                dx += *w as isize;
                if dx >= self.terminal.width as isize {
                    break;
                }
                if dx >= (self.sidebar_width + w) as isize {
                    let mut str = g.as_str();
                    let fg_color = if let Some(token) = self.buffer[line_number].syntax.get(i) {
                        match token {
                            TokenType::Normal => style::token_normal,
                            TokenType::Number => style::token_number,
                            TokenType::Match => style::token_match,
                            TokenType::String => style::token_string,
                            TokenType::MlString => style::token_ml_string,
                            TokenType::Comment => style::token_comment,
                            TokenType::MlComment => style::token_ml_comment,
                            TokenType::Keyword1 => style::token_keyword1,
                            TokenType::Keyword2 => style::token_keyword2,
                            TokenType::Keyword3 => style::token_keyword3,
                        }
                    } else {
                        style::token_normal
                    };
                    let mut bg_color = style::background;

                    if let Some((begin, end)) = self.get_selection() {
                        let current = (i, line_number).into();
                        if begin <= current && current < end {
                            bg_color = style::background_selected;
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

        if self.is_searching {
            self.render_search();
        }
    }

    pub fn check_minimum_window_size(&mut self) -> bool {
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
                cursor::MoveTo(((w - hint_0.width()) / 2) as u16, (h / 2).saturating_sub(1) as u16),
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

    fn render_sidebar(&mut self) {
        let cursor = self.get_cursor_position();
        for i in 0..(self.terminal.height.saturating_sub(2)) {
            if self.viewbox.y + i < self.buffer.len() {
                let lineno = format!(
                    "{:>width$} ",
                    i + self.viewbox.y + 1,
                    width = self.sidebar_width - 1
                );
                let num = if i + self.viewbox.y == cursor.y {
                    lineno.with(style::text_sidebar_selected)
                } else {
                    lineno.with(style::text_dimmed)
                };
                self.terminal
                    .write((0, i).into(), num.on(style::background_sidebar));
            } else {
                self.terminal.write(
                    (0, i).into(),
                    format!("{:>width$} ", "~", width = self.sidebar_width - 1)
                        .with(style::text_dimmed)
                        .on(style::background_sidebar),
                );
            }
        }
    }

    fn render_cursor(&mut self) {
        let cursor = self.get_cursor_position();
        let (x, y) = (
            cursor.x as isize - self.viewbox.x as isize + self.sidebar_width as isize,
            cursor.y as isize - self.viewbox.y as isize,
        );

        if x >= 0 && x < self.terminal.width as isize && y >= 0 && y < self.terminal.height as isize
        {
            self.terminal.cursor = Some((x as usize, y as usize).into());
        } else {
            self.terminal.cursor = None;
        }
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

    fn create_history(&mut self) {
        self.update_syntax();

        self.history
            .push_state(&self.buffer, self.viewbox, self.cursor, self.anchor);
    }
    fn update_last_history_state(&mut self) {
        self.history
            .update_state(self.viewbox, self.cursor, self.anchor);
    }

    fn trigger_copy(&mut self) -> Result<(), Error> {
        // Fix wrong deletion when selection is empty
        if let Some((begin, end)) = self.get_selection() {
            if begin == end {
                self.anchor = None;
            }
        }

        let mut clipboard = String::new();
        if let Some((begin, end)) = self.get_selection() {
            for i in begin.y..=end.y {
                let row = &self.buffer[i];
                let l = if i == begin.y { begin.x } else { 0 };
                let r = if i == end.y { end.x } else { row.len() };
                clipboard.push_str(
                    &row.rope[l..r.max(row.len())]
                        .iter()
                        .map(|(g, _)| g.as_str())
                        .collect::<String>(),
                );
                if i != end.y {
                    clipboard.push('\n');
                }
            }
        } else {
            // Just copy the current line
            clipboard = self.buffer[self.cursor.y].to_string();
        }

        terminal_clipboard::set_string(clipboard)?;

        Ok(())
    }

    fn trigger_paste(&mut self) {
        self.update_last_history_state();
        self.dirty = true;

        let clipboard = terminal_clipboard::get_string().unwrap_or_default();

        if clipboard.is_empty() {
            return;
        }

        if let Some((begin, end)) = self.get_selection() {
            self.delete_selection_range(begin, end);
        }
        let lines = clipboard
            .split('\n')
            .map(|line| line.strip_suffix('\r').unwrap_or(line))
            .collect::<Vec<&str>>();
        let line_count = lines.len();
        if line_count == 1 {
            // Paste to the current line
            let middle: Row = lines[0].into();
            let (left, right) = self.buffer[self.cursor.y].rope.split_at(self.cursor.x);
            self.buffer[self.cursor.y] = Row::from([left, &middle.rope, right].concat());
            self.cursor.x += middle.len();
        } else {
            let current_line = self.buffer[self.cursor.y].rope.clone();
            let (left, right) = current_line.split_at(self.cursor.x);
            for (i, &line) in lines.iter().enumerate() {
                let line: Row = line.into();
                if i == 0 {
                    self.buffer[self.cursor.y] = Row::from([left, &line.rope].concat());
                } else if i == line_count - 1 {
                    self.buffer
                        .insert(self.cursor.y + i, Row::from([&line.rope, right].concat()));
                    self.cursor.x = line.len();
                    self.cursor.y += i;
                } else {
                    self.buffer.insert(self.cursor.y + i, line);
                }
            }
        }

        self.create_history();
    }

    fn update_syntax(&mut self) {
        let mut state = TokenState::default();
        for line in self.buffer.iter_mut() {
            line.update_syntax(&self.syntax, &mut state);
        }
    }

    fn on_exit(&mut self) -> Result<(), Error> {
        self.terminal.cleanup()?;

        Ok(())
    }

    /// Attempts to save the file. Returns `true` if the file was saved successfully, `false` otherwise.
    fn try_save_file(&mut self, is_save_as: bool) -> Result<bool, Error> {
        self.update_last_history_state();

        if is_save_as || self.filename.is_none() {
            if let Some(ref filename) = Tui::prompt_filename(self)? {
                if Path::new(filename).is_dir() {
                    Tui::alert(
                        self,
                        "错误".to_string(),
                        "输入的文件名是一个目录".to_string(),
                    )?;
                    return Ok(false);
                }

                if Path::new(filename).exists() {
                    if let Some(false) = Tui::confirm_overwrite(self, filename)? {
                        return Ok(false);
                    }
                }

                self.filename = Some(filename.to_string());
            }
        }

        if let Some(filename) = self.filename.clone() {
            if let Err(err) = std::fs::write(
                filename,
                self.buffer
                    .iter()
                    .map(|line| line.to_string())
                    .collect::<Vec<_>>()
                    .join(if self.is_crlf { "\r\n" } else { "\n" }),
            ) {
                use std::io::ErrorKind::*;
                let err_message = match err.kind() {
                    AddrInUse => "地址被占用",
                    AddrNotAvailable => "地址不可用",
                    AlreadyExists => "文件已存在",
                    ArgumentListTooLong => "参数列表过长",
                    BrokenPipe => "管道已断开",
                    ConnectionAborted => "连接已中止",
                    ConnectionRefused => "连接被拒绝",
                    ConnectionReset => "连接已重置",
                    CrossesDevices => "不能跨设备进行链接或重命名",
                    Deadlock => "检测到死锁",
                    DirectoryNotEmpty => "文件夹不是空的，里面还有东西",
                    ExecutableFileBusy => "可执行文件正在使用中",
                    FileTooLarge => "文件太大",
                    HostUnreachable => "主机不可达",
                    Interrupted => "操作被中断",
                    InvalidData => "数据无效",
                    InvalidInput => "输入参数无效",
                    IsADirectory => "输入的文件名是一个目录",
                    NetworkDown => "网络连接已断开",
                    NetworkUnreachable => "网络不可达",
                    NotADirectory => "不是一个目录",
                    NotConnected => "未连接",
                    NotFound => "未找到文件",
                    NotSeekable => "文件不支持查找",
                    Other => "发生未知错误",
                    OutOfMemory => "内存不足（OOM）",
                    PermissionDenied => "需要管理员权限",
                    ReadOnlyFilesystem => "文件系统为只读",
                    ResourceBusy => "资源正忙",
                    StaleNetworkFileHandle => "网络文件句柄已失效",
                    StorageFull => "存储空间不足",
                    TimedOut => "操作超时",
                    UnexpectedEof => "遇到意外 EOF 结束符，拼尽全力无法战胜",
                    _ => &err.to_string(),
                };
                Tui::alert(
                    self,
                    "保存失败".to_string(),
                    "错误: ".to_string() + err_message,
                )?;
                return Ok(false);
            }

            self.dirty = false;

            self.create_history();

            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn into_search_mode(&mut self) -> Result<(), Error> {
        self.is_searching = true;

        let anchor = self.cursor;

        if self.check_minimum_window_size() {
            self.render_to_buffer();
        }

        let mut last_input = String::new();
        loop {
            if event::poll(std::time::Duration::from_millis(25))? {
                let event = event::read()?;
                // match self.search.handle_event(&event)? {
                //     Some(true) => {}
                //     Some(false) => {
                //         self.is_searching = false;
                //         return Ok(());
                //     }
                //     None => {
                //         if let Event::Mouse(event) = event {
                //             match event.kind {
                //                 MouseEventKind::Down(MouseButton::Left) => {
                //                     self.cursor = event.into();
                //                     self.anchor = Some(self.cursor);
                //                 }
                //                 MouseEventKind::Drag(MouseButton::Left) => {
                //                     self.cursor = event.into();
                //                     self.anchor = Some(self.cursor);
                //                 }
                //                 _ => {}
                //             }
                //         }
                //     }
                // }
            }

            let input = self.search.buffer.to_string();
            if input != last_input {
                self.search_result.clear();
                for line in self.buffer.iter().map(|line| line.to_string()) {
                    let mut i = 0;
                    while let Some(pos) = line[i..].find(&input) {
                        self.search_result
                            .push((line[..i].graphemes(true).count(), self.cursor.y).into());
                        i = pos + input.len();
                    }
                }
            }

            if !self.check_minimum_window_size() {
                continue;
            }

            self.render_to_buffer();

            last_input = input;
        }
    }

    fn render_search(&mut self) {
        self.search.render(&mut self.terminal);
    }
}
