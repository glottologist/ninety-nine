use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::app::AppMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    MoveUp,
    MoveDown,
    CycleSort,
    ReverseSort,
    CycleFilter,
    Enter,
    Back,
    Quit,
    None,
}

#[must_use]
pub fn handle_key_event(key: KeyEvent, mode: &AppMode) -> Action {
    if key.kind != KeyEventKind::Press {
        return Action::None;
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Action::Quit;
    }

    match mode {
        AppMode::Browse => match key.code {
            KeyCode::Char('k') | KeyCode::Up => Action::MoveUp,
            KeyCode::Char('j') | KeyCode::Down => Action::MoveDown,
            KeyCode::Char('s') => Action::CycleSort,
            KeyCode::Char('r') => Action::ReverseSort,
            KeyCode::Char('f') => Action::CycleFilter,
            KeyCode::Enter => Action::Enter,
            KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
            _ => Action::None,
        },
        AppMode::Detail(_) => match key.code {
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => Action::Back,
            _ => Action::None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use rstest::rstest;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn ctrl_c() -> KeyEvent {
        KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn release(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        }
    }

    #[rstest]
    #[case(key(KeyCode::Char('j')), Action::MoveDown)]
    #[case(key(KeyCode::Down), Action::MoveDown)]
    #[case(key(KeyCode::Char('k')), Action::MoveUp)]
    #[case(key(KeyCode::Up), Action::MoveUp)]
    #[case(key(KeyCode::Char('s')), Action::CycleSort)]
    #[case(key(KeyCode::Char('r')), Action::ReverseSort)]
    #[case(key(KeyCode::Char('f')), Action::CycleFilter)]
    #[case(key(KeyCode::Enter), Action::Enter)]
    #[case(key(KeyCode::Char('q')), Action::Quit)]
    #[case(key(KeyCode::Esc), Action::Quit)]
    #[case(key(KeyCode::Char('x')), Action::None)]
    fn browse_mode_keys(#[case] event: KeyEvent, #[case] expected: Action) {
        assert_eq!(handle_key_event(event, &AppMode::Browse), expected);
    }

    #[rstest]
    #[case(key(KeyCode::Char('q')), Action::Back)]
    #[case(key(KeyCode::Esc), Action::Back)]
    #[case(key(KeyCode::Enter), Action::Back)]
    #[case(key(KeyCode::Char('j')), Action::None)]
    fn detail_mode_keys(#[case] event: KeyEvent, #[case] expected: Action) {
        assert_eq!(handle_key_event(event, &AppMode::Detail(0)), expected);
    }

    #[test]
    fn ctrl_c_quits_in_any_mode() {
        assert_eq!(handle_key_event(ctrl_c(), &AppMode::Browse), Action::Quit);
        assert_eq!(
            handle_key_event(ctrl_c(), &AppMode::Detail(0)),
            Action::Quit
        );
    }

    #[test]
    fn release_events_are_ignored() {
        assert_eq!(
            handle_key_event(release(KeyCode::Char('q')), &AppMode::Browse),
            Action::None
        );
        assert_eq!(
            handle_key_event(release(KeyCode::Char('j')), &AppMode::Browse),
            Action::None
        );
    }
}
