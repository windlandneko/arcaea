use crate::Row;

#[derive(Debug)]
struct Diff {
    old: Vec<Option<Row>>,
    new: Vec<Option<Row>>,
    len: usize,
}

impl Diff {
    fn new(len: usize) -> Self {
        Self {
            old: vec![],
            new: vec![],
            len,
        }
    }
}

#[derive(Default)]
pub struct History {
    history: Vec<Diff>,

    pub current: Vec<Row>,

    version: usize,
}

/// A history structure that maintains a list of states and allows undo/redo operations.
///
/// The history keeps track of states through a version number, which points to the current state.
/// When undoing, the version number decreases, and when redoing, it increases.
///
/// # Examples
///
/// ```
/// let mut history = History::new();
/// history.push_state("first");  // version = 0
/// history.push_state("second"); // version = 1
///
/// assert_eq!(history.current, Some(&"second"));
///
/// history.undo(); // Goes back to "first"
/// assert_eq!(history.current, Some(&"first"));
///
/// history.redo(); // Returns to "second"
/// assert_eq!(history.current, Some(&"second"));
///
/// // If we push a new state after undoing, all future states are discarded
/// history.undo();
/// history.push_state("new_state"); // Discards "second"
/// ```
///
/// # Methods
///
/// - `new()`: Creates a new empty history
/// - `push_state()`: Adds a new state to the history
/// - `undo()`: Moves back one version in history
/// - `redo()`: Moves forward one version in history
/// - `current()`: Returns a reference to the current state
impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a new state to the history.
    /// If the current version isn't the newest, it will truncate the history to the current version.
    pub fn push_state(&mut self, item: &Vec<Row>) {
        let v = self.version;
        let old_len = self.current.len();
        let new_len = item.len();

        self.history.truncate(v);
        self.history.push(Diff::new(new_len));

        // [..., old, new] <- self.history
        //       v-1   v   <- index

        if v == 0 {
            self.current = item.clone();
        } else {
            self.history[v - 1].new.resize(new_len, None);
            self.history[v].old.resize(old_len, None);

            let min_len = old_len.min(new_len);
            for i in 0..min_len {
                let old_row = &mut self.current[i];
                let new_row = &item[i];
                if old_row != new_row {
                    self.history[v - 1].new[i] = Some(new_row.clone());
                    self.history[v].old[i] = Some(old_row.clone());
                    *old_row = new_row.clone();
                }
            }
            for i in min_len..old_len {
                self.history[v].old[i] = Some(self.current[i].clone());
            }

            self.current.resize(new_len, Row::default());
            for i in min_len..new_len {
                self.history[v - 1].new[i] = Some(item[i].clone());
                self.current[i] = item[i].clone();
            }
        }

        assert_eq!(self.current, *item);

        self.version += 1;
    }

    pub fn undo(&mut self) -> bool {
        if self.version > 1 {
            self.version -= 1;
            self.current
                .resize(self.history[self.version - 1].len, Row::default());
            for row in &self.history {
                println!("history: {:?}", row);
            }
            for (i, row) in self.history[self.version].old.iter().enumerate() {
                if let Some(row) = row {
                    self.current[i] = row.clone();
                }
            }
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if self.version < self.history.len() {
            self.current
                .resize(self.history[self.version].len, Row::default());
            for (i, row) in self.history[self.version - 1].new.iter().enumerate() {
                if let Some(row) = row {
                    self.current[i] = row.clone();
                }
            }
            self.version += 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history() {
        let mut history = History::new();
        let ver0 = vec!["QwQ".into(), "version = 0".into(), "unchanged".into()];
        let ver1 = vec![
            "awa".into(),
            "version = 1".into(),
            "changed".into(),
            "very".into(),
            "long".into(),
        ];
        let ver2 = vec!["QwQ".into(), "version = 2".into(), "changed".into()];

        history.push_state(&ver0);
        history.push_state(&ver1);
        history.push_state(&ver2);

        assert_eq!(history.current, ver2);
        assert_eq!(history.redo(), false);
        assert_eq!(history.undo(), true);   // 2 -> 1
        assert_eq!(history.current, ver1);
        assert_eq!(history.redo(), true);   // 1 -> 2
        assert_eq!(history.current, ver2);
        assert_eq!(history.redo(), false);

        assert_eq!(history.undo(), true);   // 2 -> 1
        assert_eq!(history.undo(), true);   // 1 -> 0
        assert_eq!(history.undo(), false);

        history.push_state(&vec!["TvT".into()]); // version = 1, drops old version 1 and 2
        assert_eq!(history.current, vec!["TvT".into()]);
    }
}
