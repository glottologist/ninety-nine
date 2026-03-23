use crate::types::{
    FailurePattern, FlakinessCategory, FlakinessScore, RunSession, TestRun, TrendSummary,
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

#[derive(Debug)]
pub struct Cursor {
    position: usize,
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

impl Cursor {
    #[must_use]
    pub const fn new() -> Self {
        Self { position: 0 }
    }

    #[must_use]
    pub const fn position(&self) -> usize {
        self.position
    }

    pub const fn move_up(&mut self) {
        self.position = self.position.saturating_sub(1);
    }

    pub fn move_down(&mut self, item_count: usize) {
        if item_count > 0 {
            self.position = self.position.saturating_add(1).min(item_count - 1);
        }
    }

    pub const fn reset(&mut self) {
        self.position = 0;
    }
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

pub struct ScoresApp {
    pub scores: Vec<FlakinessScore>,
    pub filtered: Vec<usize>,
    pub cursor: Cursor,
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
        let mut app = Self {
            scores,
            filtered,
            cursor: Cursor::new(),
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

    pub const fn move_up(&mut self) {
        self.cursor.move_up();
    }

    pub fn move_down(&mut self) {
        self.cursor.move_down(self.filtered.len());
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
        self.cursor.reset();
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
        self.filtered
            .get(self.cursor.position())
            .map(|&i| &self.scores[i])
    }

    pub fn enter_detail(&mut self, detail: DetailData) {
        self.detail = Some(detail);
        self.mode = AppMode::Detail(self.cursor.position());
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

pub struct HistoryApp {
    pub sessions: Vec<RunSession>,
    pub cursor: Cursor,
}

impl HistoryApp {
    #[must_use]
    pub const fn new(sessions: Vec<RunSession>) -> Self {
        Self {
            sessions,
            cursor: Cursor::new(),
        }
    }

    pub const fn move_up(&mut self) {
        self.cursor.move_up();
    }

    pub fn move_down(&mut self) {
        self.cursor.move_down(self.sessions.len());
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
        assert_eq!(app.cursor.position(), 0);
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
        app.cursor.move_down(app.filtered.len());
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
                prop_assert_eq!(app.cursor.position(), 0);
            } else {
                prop_assert!(app.cursor.position() < app.filtered.len());
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
                prop_assert_eq!(app.cursor.position(), 0);
            } else {
                prop_assert!(app.cursor.position() < app.sessions.len());
            }
        }
    }
}
