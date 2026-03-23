pub mod app;
pub mod input;
pub mod render;

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::{self, Event, poll};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::runtime::Handle;

use crate::analysis::pattern::detect_patterns;
use crate::analysis::trend::calculate_trend;
use crate::config::model::Config;
use crate::error::NinetyNineError;
use crate::storage::{Storage, StorageBackend};
use crate::types::{FlakinessScore, RunSession};

use self::app::{AppMode, DetailData, HistoryApp, ScoresApp};
use self::input::{Action, handle_key_event};

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl TerminalGuard {
    fn new() -> Result<Self, NinetyNineError> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if let Err(e) = disable_raw_mode() {
            tracing::warn!("failed to disable raw mode: {e}");
        }
        if let Err(e) = execute!(self.terminal.backend_mut(), LeaveAlternateScreen) {
            tracing::warn!("failed to leave alternate screen: {e}");
        }
        if let Err(e) = self.terminal.show_cursor() {
            tracing::warn!("failed to show cursor: {e}");
        }
    }
}

fn install_panic_hook() {
    use std::sync::Once;
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let original = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
            original(info);
        }));
    });
}

fn install_signal_handlers() -> Result<Arc<AtomicBool>, NinetyNineError> {
    let shutdown = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&shutdown))?;
    signal_hook::flag::register(signal_hook::consts::SIGHUP, Arc::clone(&shutdown))?;
    Ok(shutdown)
}

/// Launches the interactive TUI for flakiness scores.
///
/// # Errors
///
/// Returns an error if terminal setup fails or a storage query fails.
pub fn run_scores(
    scores: Vec<FlakinessScore>,
    confidence_threshold: f64,
    storage: &StorageBackend,
    config: &Config,
) -> Result<(), NinetyNineError> {
    let handle = Handle::current();
    tokio::task::block_in_place(|| {
        scores_loop(scores, confidence_threshold, storage, config, &handle)
    })
}

fn scores_loop(
    scores: Vec<FlakinessScore>,
    confidence_threshold: f64,
    storage: &StorageBackend,
    config: &Config,
    handle: &Handle,
) -> Result<(), NinetyNineError> {
    install_panic_hook();
    let shutdown = install_signal_handlers()?;
    let mut guard = TerminalGuard::new()?;
    let mut app = ScoresApp::new(scores, confidence_threshold);

    loop {
        guard.terminal.draw(|f| render::draw_scores(f, &app))?;

        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        if poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match handle_key_event(key, &app.mode) {
                    Action::MoveUp => app.move_up(),
                    Action::MoveDown => app.move_down(),
                    Action::CycleSort => app.cycle_sort(),
                    Action::ReverseSort => app.reverse_sort(),
                    Action::CycleFilter => app.cycle_filter(),
                    Action::Enter => {
                        if let Some(score) = app.selected_score() {
                            let name = score.test_name.as_ref();
                            let detail =
                                fetch_detail(handle, storage, name, config.detection.window_size)?;
                            app.enter_detail(detail);
                        }
                    }
                    Action::Back => app.exit_detail(),
                    Action::Quit => break,
                    Action::None => {}
                }
            }
        }
    }

    Ok(())
}

/// Launches the interactive TUI for session history.
///
/// # Errors
///
/// Returns an error if terminal setup fails.
pub fn run_history(sessions: Vec<RunSession>) -> Result<(), NinetyNineError> {
    tokio::task::block_in_place(|| history_loop(sessions))
}

fn history_loop(sessions: Vec<RunSession>) -> Result<(), NinetyNineError> {
    install_panic_hook();
    let shutdown = install_signal_handlers()?;
    let mut guard = TerminalGuard::new()?;
    let mut app = HistoryApp::new(sessions);

    loop {
        guard.terminal.draw(|f| render::draw_history(f, &app))?;

        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        if poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match handle_key_event(key, &AppMode::Browse) {
                    Action::MoveUp => app.move_up(),
                    Action::MoveDown => app.move_down(),
                    Action::Quit => break,
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn fetch_detail(
    handle: &Handle,
    storage: &StorageBackend,
    test_name: &str,
    window_size: u32,
) -> Result<DetailData, NinetyNineError> {
    handle.block_on(async {
        let runs = storage.get_test_runs(test_name, 20).await?;
        let trend = calculate_trend(test_name, &runs, window_size);
        let patterns = detect_patterns(&runs);
        Ok(DetailData {
            runs,
            trend,
            patterns,
        })
    })
}
