use crossterm::{
    cursor, event, execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, BeginSynchronizedUpdate, EndSynchronizedUpdate,
        EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use std::io::{stdout, Result};

pub fn get_window_size() -> Result<(u16, u16)> {
    crossterm::terminal::size()
}

pub fn init() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, event::EnableMouseCapture)?;
    Ok(())
}

pub fn cleanup() -> Result<()> {
    disable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, event::DisableMouseCapture, LeaveAlternateScreen)?;
    Ok(())
}

pub fn render_begin() -> Result<()> {
    let mut stdout = stdout();
    execute!(
        stdout,
        BeginSynchronizedUpdate,
        cursor::SavePosition,
        cursor::Hide,
    )?;
    Ok(())
}

pub fn render_end() -> Result<()> {
    let mut stdout = stdout();
    execute!(
        stdout,
        cursor::Show,
        cursor::RestorePosition,
        EndSynchronizedUpdate,
    )?;
    Ok(())
}
