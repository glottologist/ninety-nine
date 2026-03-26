use ratatui::widgets::TableState;

use crate::types::{
    FailurePattern, FlakinessCategory, FlakinessScore, RunSession, TestOutcome, TestRun,
    TrendSummary,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortField {
    Name,
    Runs,
    PassRate,
    PFlaky,
    Category,
}

impl SortField {
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Name => Self::Runs,
            Self::Runs => Self::PassRate,
            Self::PassRate => Self::PFlaky,
            Self::PFlaky => Self::Category,
            Self::Category => Self::Name,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Name => "Test",
            Self::Runs => "Runs",
            Self::PassRate => "Pass%",
            Self::PFlaky => "P(flaky)",
            Self::Category => "Category",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionSortField {
    Name,
    Outcome,
    Duration,
    Retries,
}

impl SessionSortField {
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Name => Self::Outcome,
            Self::Outcome => Self::Duration,
            Self::Duration => Self::Retries,
            Self::Retries => Self::Name,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Name => "Test",
            Self::Outcome => "Outcome",
            Self::Duration => "Duration",
            Self::Retries => "Retries",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMode {
    Browse,
    Detail(usize),
}

pub struct DetailData {
    pub runs: Vec<TestRun>,
    pub trend: Option<TrendSummary>,
    pub patterns: Vec<FailurePattern>,
}

const fn category_ordinal(cat: FlakinessCategory) -> u8 {
    match cat {
        FlakinessCategory::Stable => 0,
        FlakinessCategory::Occasional => 1,
        FlakinessCategory::Moderate => 2,
        FlakinessCategory::Frequent => 3,
        FlakinessCategory::Critical => 4,
    }
}

const fn outcome_ordinal(outcome: TestOutcome) -> u8 {
    match outcome {
        TestOutcome::Passed => 0,
        TestOutcome::Failed => 1,
        TestOutcome::Timeout => 2,
        TestOutcome::Panic => 3,
        TestOutcome::Ignored => 4,
    }
}

pub struct ScoresApp {
    pub scores: Vec<FlakinessScore>,
    pub filtered: Vec<usize>,
    pub table_state: TableState,
    pub sort_field: SortField,
    pub sort_ascending: bool,
    pub filter_category: Option<FlakinessCategory>,
    pub mode: AppMode,
    pub confidence_threshold: f64,
    pub detail: Option<DetailData>,
}

impl ScoresApp {
    #[must_use]
    pub fn new(scores: Vec<FlakinessScore>, confidence_threshold: f64) -> Self {
        let filtered: Vec<usize> = (0..scores.len()).collect();
        let initial_selected = if scores.is_empty() { None } else { Some(0) };
        let mut app = Self {
            scores,
            filtered,
            table_state: TableState::new().with_selected(initial_selected),
            sort_field: SortField::PFlaky,
            sort_ascending: false,
            filter_category: None,
            mode: AppMode::Browse,
            confidence_threshold,
            detail: None,
        };
        app.sort_filtered();
        app
    }

    pub fn move_up(&mut self) {
        if !self.filtered.is_empty() {
            self.table_state.select_previous();
            self.clamp_selection();
        }
    }

    pub fn move_down(&mut self) {
        if !self.filtered.is_empty() {
            self.table_state.select_next();
            self.clamp_selection();
        }
    }

    fn clamp_selection(&mut self) {
        if let Some(sel) = self.table_state.selected() {
            let max = self.filtered.len().saturating_sub(1);
            if sel > max {
                self.table_state.select(Some(max));
            }
        }
    }

    pub fn cycle_sort(&mut self) {
        self.sort_field = self.sort_field.next();
        self.sort_filtered();
    }

    pub fn reverse_sort(&mut self) {
        self.sort_ascending = !self.sort_ascending;
        self.sort_filtered();
    }

    pub fn cycle_filter(&mut self) {
        self.filter_category = match self.filter_category {
            None => Some(FlakinessCategory::Stable),
            Some(FlakinessCategory::Stable) => Some(FlakinessCategory::Occasional),
            Some(FlakinessCategory::Occasional) => Some(FlakinessCategory::Moderate),
            Some(FlakinessCategory::Moderate) => Some(FlakinessCategory::Frequent),
            Some(FlakinessCategory::Frequent) => Some(FlakinessCategory::Critical),
            Some(FlakinessCategory::Critical) => None,
        };
        self.rebuild_filtered();
    }

    fn rebuild_filtered(&mut self) {
        let threshold = self.confidence_threshold;
        self.filtered = (0..self.scores.len())
            .filter(|&i| match self.filter_category {
                None => true,
                Some(cat) => {
                    let effective = self.scores[i].effective_score(threshold);
                    FlakinessCategory::from_score(effective) == cat
                }
            })
            .collect();
        self.sort_filtered();
        let sel = if self.filtered.is_empty() {
            None
        } else {
            Some(0)
        };
        self.table_state.select(sel);
    }

    pub fn sort_filtered(&mut self) {
        let threshold = self.confidence_threshold;
        let ascending = self.sort_ascending;
        let field = self.sort_field;

        self.filtered.sort_by(|&a, &b| {
            let sa = &self.scores[a];
            let sb = &self.scores[b];
            let ord = match field {
                SortField::Name => sa.test_name.as_ref().cmp(sb.test_name.as_ref()),
                SortField::Runs => sa.total_runs.cmp(&sb.total_runs),
                SortField::PassRate => sa
                    .pass_rate
                    .partial_cmp(&sb.pass_rate)
                    .unwrap_or(std::cmp::Ordering::Equal),
                SortField::PFlaky => sa
                    .effective_score(threshold)
                    .partial_cmp(&sb.effective_score(threshold))
                    .unwrap_or(std::cmp::Ordering::Equal),
                SortField::Category => {
                    let ca = FlakinessCategory::from_score(sa.effective_score(threshold));
                    let cb = FlakinessCategory::from_score(sb.effective_score(threshold));
                    category_ordinal(ca).cmp(&category_ordinal(cb))
                }
            };
            if ascending { ord } else { ord.reverse() }
        });
    }

    #[must_use]
    pub fn selected_score(&self) -> Option<&FlakinessScore> {
        self.table_state
            .selected()
            .and_then(|i| self.filtered.get(i))
            .map(|&i| &self.scores[i])
    }

    #[must_use]
    pub fn selected_index(&self) -> usize {
        self.table_state.selected().unwrap_or(0)
    }

    pub fn enter_detail(&mut self, detail: DetailData) {
        self.detail = Some(detail);
        self.mode = AppMode::Detail(self.selected_index());
    }

    pub fn exit_detail(&mut self) {
        self.detail = None;
        self.mode = AppMode::Browse;
    }

    #[must_use]
    pub const fn filter_label(&self) -> &str {
        match self.filter_category {
            None => "All",
            Some(cat) => cat.label(),
        }
    }
}

pub struct SessionDetail {
    pub runs: Vec<TestRun>,
    pub filtered: Vec<usize>,
    pub table_state: TableState,
    pub sort_field: SessionSortField,
    pub sort_ascending: bool,
    pub filter_outcome: Option<TestOutcome>,
}

impl SessionDetail {
    #[must_use]
    pub fn new(runs: Vec<TestRun>) -> Self {
        let filtered: Vec<usize> = (0..runs.len()).collect();
        let initial = if runs.is_empty() { None } else { Some(0) };
        let mut detail = Self {
            runs,
            filtered,
            table_state: TableState::new().with_selected(initial),
            sort_field: SessionSortField::Name,
            sort_ascending: true,
            filter_outcome: None,
        };
        detail.sort_filtered();
        detail
    }

    pub fn move_up(&mut self) {
        if !self.filtered.is_empty() {
            self.table_state.select_previous();
            self.clamp_selection();
        }
    }

    pub fn move_down(&mut self) {
        if !self.filtered.is_empty() {
            self.table_state.select_next();
            self.clamp_selection();
        }
    }

    fn clamp_selection(&mut self) {
        if let Some(sel) = self.table_state.selected() {
            let max = self.filtered.len().saturating_sub(1);
            if sel > max {
                self.table_state.select(Some(max));
            }
        }
    }

    pub fn cycle_sort(&mut self) {
        self.sort_field = self.sort_field.next();
        self.sort_filtered();
    }

    pub fn reverse_sort(&mut self) {
        self.sort_ascending = !self.sort_ascending;
        self.sort_filtered();
    }

    pub fn cycle_filter(&mut self) {
        self.filter_outcome = match self.filter_outcome {
            None => Some(TestOutcome::Passed),
            Some(TestOutcome::Passed) => Some(TestOutcome::Failed),
            Some(TestOutcome::Failed) => Some(TestOutcome::Timeout),
            Some(TestOutcome::Timeout) => Some(TestOutcome::Panic),
            Some(TestOutcome::Panic) => Some(TestOutcome::Ignored),
            Some(TestOutcome::Ignored) => None,
        };
        self.rebuild_filtered();
    }

    fn rebuild_filtered(&mut self) {
        self.filtered = (0..self.runs.len())
            .filter(|&i| match self.filter_outcome {
                None => true,
                Some(outcome) => self.runs[i].outcome == outcome,
            })
            .collect();
        self.sort_filtered();
        let sel = if self.filtered.is_empty() {
            None
        } else {
            Some(0)
        };
        self.table_state.select(sel);
    }

    pub fn sort_filtered(&mut self) {
        let ascending = self.sort_ascending;
        let field = self.sort_field;

        self.filtered.sort_by(|&a, &b| {
            let ra = &self.runs[a];
            let rb = &self.runs[b];
            let ord = match field {
                SessionSortField::Name => ra.test_name.as_ref().cmp(rb.test_name.as_ref()),
                SessionSortField::Outcome => {
                    outcome_ordinal(ra.outcome).cmp(&outcome_ordinal(rb.outcome))
                }
                SessionSortField::Duration => ra.duration.cmp(&rb.duration),
                SessionSortField::Retries => ra.retry_count.cmp(&rb.retry_count),
            };
            if ascending { ord } else { ord.reverse() }
        });
    }

    #[must_use]
    pub const fn filter_label(&self) -> &str {
        match self.filter_outcome {
            None => "All",
            Some(TestOutcome::Passed) => "Pass",
            Some(TestOutcome::Failed) => "Fail",
            Some(TestOutcome::Timeout) => "Timeout",
            Some(TestOutcome::Panic) => "Panic",
            Some(TestOutcome::Ignored) => "Ignored",
        }
    }
}

pub struct HistoryApp {
    pub sessions: Vec<RunSession>,
    pub table_state: TableState,
    pub mode: AppMode,
    pub detail: Option<SessionDetail>,
}

impl HistoryApp {
    #[must_use]
    pub fn new(sessions: Vec<RunSession>) -> Self {
        let initial = if sessions.is_empty() { None } else { Some(0) };
        Self {
            sessions,
            table_state: TableState::new().with_selected(initial),
            mode: AppMode::Browse,
            detail: None,
        }
    }

    pub fn move_up(&mut self) {
        if !self.sessions.is_empty() {
            self.table_state.select_previous();
            self.clamp_selection();
        }
    }

    pub fn move_down(&mut self) {
        if !self.sessions.is_empty() {
            self.table_state.select_next();
            self.clamp_selection();
        }
    }

    fn clamp_selection(&mut self) {
        if let Some(sel) = self.table_state.selected() {
            let max = self.sessions.len().saturating_sub(1);
            if sel > max {
                self.table_state.select(Some(max));
            }
        }
    }

    #[must_use]
    pub fn selected_session(&self) -> Option<&RunSession> {
        self.table_state
            .selected()
            .and_then(|i| self.sessions.get(i))
    }

    pub fn enter_detail(&mut self, detail: SessionDetail) {
        let idx = self.table_state.selected().unwrap_or(0);
        self.detail = Some(detail);
        self.mode = AppMode::Detail(idx);
    }

    pub fn exit_detail(&mut self) {
        self.detail = None;
        self.mode = AppMode::Browse;
    }

    pub fn detail_move_up(&mut self) {
        if let Some(detail) = &mut self.detail {
            detail.move_up();
        }
    }

    pub fn detail_move_down(&mut self) {
        if let Some(detail) = &mut self.detail {
            detail.move_down();
        }
    }

    pub fn detail_cycle_sort(&mut self) {
        if let Some(detail) = &mut self.detail {
            detail.cycle_sort();
        }
    }

    pub fn detail_reverse_sort(&mut self) {
        if let Some(detail) = &mut self.detail {
            detail.reverse_sort();
        }
    }

    pub fn detail_cycle_filter(&mut self) {
        if let Some(detail) = &mut self.detail {
            detail.cycle_filter();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::test_score;
    use proptest::prelude::*;

    fn sample_scores() -> Vec<FlakinessScore> {
        vec![
            test_score("tests::stable", 0.005),
            test_score("tests::occasional", 0.03),
            test_score("tests::moderate", 0.10),
            test_score("tests::frequent", 0.25),
            test_score("tests::critical", 0.50),
        ]
    }

    #[test]
    fn new_creates_all_indices() {
        let app = ScoresApp::new(sample_scores(), 0.95);
        assert_eq!(app.filtered.len(), 5);
        assert_eq!(app.table_state.selected(), Some(0));
    }

    #[test]
    fn filter_by_category_matches_expected() {
        let mut app = ScoresApp::new(sample_scores(), 0.95);
        app.cycle_filter();
        assert_eq!(app.filter_category, Some(FlakinessCategory::Stable));
        assert_eq!(app.filtered.len(), 1);
        let score = &app.scores[app.filtered[0]];
        let cat = FlakinessCategory::from_score(score.effective_score(app.confidence_threshold));
        assert_eq!(cat, FlakinessCategory::Stable);
    }

    #[test]
    fn category_sort_orders_by_severity() {
        let mut app = ScoresApp::new(sample_scores(), 0.95);
        app.sort_field = SortField::Category;
        app.sort_ascending = true;
        app.sort_filtered();
        let cats: Vec<FlakinessCategory> = app
            .filtered
            .iter()
            .map(|&i| {
                FlakinessCategory::from_score(
                    app.scores[i].effective_score(app.confidence_threshold),
                )
            })
            .collect();
        for window in cats.windows(2) {
            assert!(category_ordinal(window[0]) <= category_ordinal(window[1]));
        }
    }

    #[test]
    fn enter_exit_detail_roundtrip() {
        let mut app = ScoresApp::new(sample_scores(), 0.95);
        app.move_down();
        let detail = DetailData {
            runs: Vec::new(),
            trend: None,
            patterns: Vec::new(),
        };
        app.enter_detail(detail);
        assert_eq!(app.mode, AppMode::Detail(1));
        assert!(app.detail.is_some());

        app.exit_detail();
        assert_eq!(app.mode, AppMode::Browse);
        assert!(app.detail.is_none());
    }

    proptest! {
        #[test]
        fn cursor_never_exceeds_bounds(
            use_empty in proptest::bool::ANY,
            moves_down in 0u32..200,
            moves_up in 0u32..200,
        ) {
            let scores = if use_empty { Vec::new() } else { sample_scores() };
            let mut app = ScoresApp::new(scores, 0.95);
            for _ in 0..moves_down {
                app.move_down();
            }
            for _ in 0..moves_up {
                app.move_up();
            }
            if app.filtered.is_empty() {
                prop_assert!(app.table_state.selected().is_none());
            } else {
                let sel = app.table_state.selected().unwrap_or(0);
                prop_assert!(sel < app.filtered.len());
            }
        }

        #[test]
        fn sort_field_cycles_back(n in 1u32..20) {
            let mut app = ScoresApp::new(sample_scores(), 0.95);
            let initial = app.sort_field;
            for _ in 0..(n * 5) {
                app.cycle_sort();
            }
            prop_assert_eq!(app.sort_field, initial);
        }

        #[test]
        fn filter_category_cycles_back(n in 1u32..20) {
            let mut app = ScoresApp::new(sample_scores(), 0.95);
            prop_assert_eq!(app.filter_category, None);
            for _ in 0..(n * 6) {
                app.cycle_filter();
            }
            prop_assert_eq!(app.filter_category, None);
        }

        #[test]
        fn sort_by_name_produces_ordered_output(ascending in proptest::bool::ANY) {
            let mut app = ScoresApp::new(sample_scores(), 0.95);
            app.sort_field = SortField::Name;
            app.sort_ascending = ascending;
            app.sort_filtered();
            let names: Vec<&str> = app
                .filtered
                .iter()
                .map(|&i| app.scores[i].test_name.as_ref())
                .collect();
            for window in names.windows(2) {
                if ascending {
                    prop_assert!(window[0] <= window[1]);
                } else {
                    prop_assert!(window[0] >= window[1]);
                }
            }
        }

        #[test]
        fn history_cursor_never_exceeds_bounds(
            use_empty in proptest::bool::ANY,
            moves_down in 0u32..200,
            moves_up in 0u32..200,
        ) {
            let sessions = if use_empty {
                Vec::new()
            } else {
                vec![
                    crate::test_helpers::test_session("abc", "main"),
                    crate::test_helpers::test_session("def", "develop"),
                ]
            };
            let mut app = HistoryApp::new(sessions);
            for _ in 0..moves_down {
                app.move_down();
            }
            for _ in 0..moves_up {
                app.move_up();
            }
            if app.sessions.is_empty() {
                prop_assert!(app.table_state.selected().is_none());
            } else {
                let sel = app.table_state.selected().unwrap_or(0);
                prop_assert!(sel < app.sessions.len());
            }
        }
    }
}
