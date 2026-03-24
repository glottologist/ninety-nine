use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table,
};

use crate::types::{FlakinessCategory, FlakinessScore, RunSession, TestOutcome};

use super::app::{AppMode, DetailData, HistoryApp, ScoresApp, SessionDetail};

const STYLE_BOLD: Style = Style::new().add_modifier(Modifier::BOLD);
const STYLE_MUTED: Style = Style::new().fg(Color::DarkGray);
const STYLE_SELECTED: Style = Style::new()
    .bg(Color::DarkGray)
    .add_modifier(Modifier::BOLD);
const STYLE_FILTER: Style = Style::new().fg(Color::Rgb(255, 165, 0));

pub fn draw_scores(f: &mut Frame, app: &mut ScoresApp) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .split(f.area());

    draw_scores_header(f, app, chunks[0]);
    draw_filter_bar(f, app, chunks[1]);
    draw_scores_table(f, app, chunks[2]);
    draw_scores_footer(f, chunks[3]);

    if let AppMode::Detail(_) = app.mode {
        if let (Some(score), Some(detail)) = (app.selected_score(), app.detail.as_ref()) {
            draw_detail_overlay(f, score, detail, app.confidence_threshold);
        }
    }
}

fn draw_scores_header(f: &mut Frame, app: &ScoresApp, area: Rect) {
    let summary = format!(
        " cargo ninety-nine | {}/{} tests shown",
        app.filtered.len(),
        app.scores.len()
    );
    let block = Block::bordered()
        .title(" Flaky Test Report ")
        .title_alignment(Alignment::Left)
        .border_style(Style::new().fg(Color::Cyan));
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(Paragraph::new(summary), inner);
}

fn draw_filter_bar(f: &mut Frame, app: &ScoresApp, area: Rect) {
    let direction = if app.sort_ascending { "asc" } else { "desc" };
    let text = Line::from(vec![
        Span::styled("Filter: ", STYLE_FILTER),
        Span::styled(
            app.filter_label(),
            STYLE_FILTER.add_modifier(Modifier::BOLD),
        ),
        Span::styled(" | ", STYLE_MUTED),
        Span::styled("Sort: ", STYLE_FILTER),
        Span::styled(
            format!("{} ({})", app.sort_field.label(), direction),
            STYLE_FILTER.add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(text), area);
}

fn draw_scores_table(f: &mut Frame, app: &mut ScoresApp, area: Rect) {
    let block = Block::bordered()
        .title(" Tests ")
        .title_alignment(Alignment::Left)
        .border_style(Style::new().fg(Color::DarkGray));
    let table_area = block.inner(area);

    let header = Row::new(vec![
        "Test",
        "Runs",
        "Pass%",
        "P(flaky)",
        "Category",
        "Confidence",
    ])
    .style(STYLE_BOLD)
    .bottom_margin(1);

    let threshold = app.confidence_threshold;
    let rows: Vec<Row> = app
        .filtered
        .iter()
        .map(|&idx| {
            let s = &app.scores[idx];
            let effective = s.effective_score(threshold);
            let cat = FlakinessCategory::from_score(effective);

            Row::new(vec![
                Cell::from(s.test_name.as_ref()),
                Cell::from(s.total_runs.to_string()),
                Cell::from(format!("{:.1}%", s.pass_rate * 100.0)),
                Cell::from(format!("{effective:.3}")),
                Cell::from(cat.label()).style(category_style(cat)),
                Cell::from(format!("{:.2}", s.confidence)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Min(30),
        Constraint::Length(6),
        Constraint::Length(7),
        Constraint::Length(9),
        Constraint::Length(12),
        Constraint::Length(10),
    ];

    let row_count = rows.len();
    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(STYLE_SELECTED)
        .block(block);

    f.render_stateful_widget(table, area, &mut app.table_state);

    let mut scrollbar_state =
        ScrollbarState::new(row_count).position(app.table_state.selected().unwrap_or(0));
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("^"))
            .end_symbol(Some("v")),
        table_area,
        &mut scrollbar_state,
    );
}

fn draw_scores_footer(f: &mut Frame, area: Rect) {
    let keys = Line::from(vec![
        Span::styled("j/k", STYLE_BOLD),
        Span::styled(":nav  ", STYLE_MUTED),
        Span::styled("s", STYLE_BOLD),
        Span::styled(":sort  ", STYLE_MUTED),
        Span::styled("r", STYLE_BOLD),
        Span::styled(":reverse  ", STYLE_MUTED),
        Span::styled("f", STYLE_BOLD),
        Span::styled(":filter  ", STYLE_MUTED),
        Span::styled("Enter", STYLE_BOLD),
        Span::styled(":detail  ", STYLE_MUTED),
        Span::styled("q", STYLE_BOLD),
        Span::styled(":quit", STYLE_MUTED),
    ]);
    f.render_widget(Paragraph::new(keys), area);
}

const fn category_style(cat: FlakinessCategory) -> Style {
    match cat {
        FlakinessCategory::Stable => Style::new().fg(Color::Green),
        FlakinessCategory::Occasional => Style::new().fg(Color::Yellow),
        FlakinessCategory::Moderate => Style::new().fg(Color::Red),
        FlakinessCategory::Frequent => Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
        FlakinessCategory::Critical => Style::new()
            .fg(Color::White)
            .bg(Color::Red)
            .add_modifier(Modifier::BOLD),
    }
}

fn draw_detail_overlay(
    f: &mut Frame,
    score: &FlakinessScore,
    detail: &DetailData,
    confidence_threshold: f64,
) {
    let area = centered_rect(70, 80, f.area());
    f.render_widget(Clear, area);

    let block = Block::bordered()
        .title(format!(" {} ", score.test_name))
        .title_alignment(Alignment::Center)
        .border_style(Style::new().fg(Color::Cyan));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::with_capacity(20);
    append_score_summary(&mut lines, score, confidence_threshold);
    append_bayesian_params(&mut lines, score);
    append_trend_section(&mut lines, detail);
    append_pattern_section(&mut lines, detail);
    append_recent_runs(&mut lines, detail);

    f.render_widget(Paragraph::new(lines), inner);
}

fn append_score_summary(
    lines: &mut Vec<Line<'_>>,
    score: &FlakinessScore,
    confidence_threshold: f64,
) {
    let effective = score.effective_score(confidence_threshold);
    let cat = FlakinessCategory::from_score(effective);
    lines.push(Line::from(format!(
        "Category: {}  |  P(flaky): {effective:.4}  |  Confidence: {:.2}",
        cat.label(),
        score.confidence
    )));
    lines.push(Line::from(format!(
        "Pass rate: {:.1}%  |  Fail rate: {:.1}%  |  Runs: {}",
        score.pass_rate * 100.0,
        score.fail_rate * 100.0,
        score.total_runs
    )));
    lines.push(Line::from(""));
}

fn append_bayesian_params(lines: &mut Vec<Line<'_>>, score: &FlakinessScore) {
    lines.push(Line::from("Bayesian Parameters:").style(STYLE_BOLD));
    let bp = &score.bayesian_params;
    lines.push(Line::from(format!(
        "  alpha: {:.2}  beta: {:.2}  posterior mean: {:.4}",
        bp.alpha, bp.beta, bp.posterior_mean
    )));
    lines.push(Line::from(format!(
        "  credible interval: [{:.4}, {:.4}]",
        bp.credible_interval_lower, bp.credible_interval_upper
    )));
    lines.push(Line::from(""));
}

fn append_trend_section(lines: &mut Vec<Line<'_>>, detail: &DetailData) {
    if let Some(trend) = &detail.trend {
        lines.push(Line::from("Trend:").style(STYLE_BOLD));
        lines.push(Line::from(format!(
            "  {}  {:.1}% -> {:.1}% (delta: {:+.1}%)",
            trend.direction,
            trend.previous_score * 100.0,
            trend.recent_score * 100.0,
            trend.score_delta * 100.0,
        )));
        lines.push(Line::from(""));
    }
}

fn append_pattern_section(lines: &mut Vec<Line<'_>>, detail: &DetailData) {
    if !detail.patterns.is_empty() {
        lines.push(Line::from("Failure Patterns:").style(STYLE_BOLD));
        for p in &detail.patterns {
            lines.push(Line::from(format!(
                "  [{:.0}% corr] {} — {}",
                p.correlation * 100.0,
                p.pattern_type,
                p.examples.first().map_or("", |s| s.as_str())
            )));
        }
        lines.push(Line::from(""));
    }
}

fn append_recent_runs(lines: &mut Vec<Line<'_>>, detail: &DetailData) {
    if !detail.runs.is_empty() {
        lines.push(Line::from("Recent Runs:").style(STYLE_BOLD));
        for run in detail.runs.iter().take(10) {
            let (symbol, _) = outcome_label_style(run.outcome);
            lines.push(Line::from(format!(
                "  [{}] {:>6.1}ms  {}",
                symbol,
                run.duration.as_secs_f64() * 1000.0,
                run.timestamp.format("%Y-%m-%d %H:%M")
            )));
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}

pub fn draw_history(f: &mut Frame, app: &mut HistoryApp) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .split(f.area());

    let header_block = Block::bordered()
        .title(" Session History ")
        .title_alignment(Alignment::Left)
        .border_style(Style::new().fg(Color::Cyan));
    let header_inner = header_block.inner(chunks[0]);
    f.render_widget(header_block, chunks[0]);
    f.render_widget(
        Paragraph::new(format!(
            " cargo ninety-nine | {} sessions",
            app.sessions.len()
        )),
        header_inner,
    );

    let content_block = Block::bordered()
        .title(" Sessions ")
        .title_alignment(Alignment::Left)
        .border_style(Style::new().fg(Color::DarkGray));
    let table_area = content_block.inner(chunks[1]);

    let header = Row::new(vec!["Date", "Tests", "Flaky", "Branch", "Commit"])
        .style(STYLE_BOLD)
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .sessions
        .iter()
        .map(|s| {
            Row::new(vec![
                Cell::from(s.started_at.format("%Y-%m-%d %H:%M").to_string()),
                Cell::from(s.test_count.to_string()),
                Cell::from(s.flaky_count.to_string()),
                Cell::from(s.branch.as_str()),
                Cell::from(s.commit_hash.get(..8).unwrap_or(&s.commit_hash)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(18),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Min(15),
        Constraint::Length(10),
    ];

    let row_count = rows.len();
    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(STYLE_SELECTED)
        .block(content_block);

    f.render_stateful_widget(table, chunks[1], &mut app.table_state);

    let mut scrollbar_state =
        ScrollbarState::new(row_count).position(app.table_state.selected().unwrap_or(0));
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("^"))
            .end_symbol(Some("v")),
        table_area,
        &mut scrollbar_state,
    );

    let keys = Line::from(vec![
        Span::styled("j/k", STYLE_BOLD),
        Span::styled(":nav  ", STYLE_MUTED),
        Span::styled("Enter", STYLE_BOLD),
        Span::styled(":detail  ", STYLE_MUTED),
        Span::styled("q", STYLE_BOLD),
        Span::styled(":quit", STYLE_MUTED),
    ]);
    f.render_widget(Paragraph::new(keys), chunks[2]);

    if let AppMode::Detail(_) = app.mode {
        if let Some(session) = app.selected_session().cloned() {
            if let Some(detail) = &mut app.detail {
                draw_session_detail_overlay(f, &session, detail);
            }
        }
    }
}

fn draw_session_detail_overlay(f: &mut Frame, session: &RunSession, detail: &mut SessionDetail) {
    let area = centered_rect(80, 85, f.area());
    f.render_widget(Clear, area);

    let commit_short = session.commit_hash.get(..8).unwrap_or(&session.commit_hash);
    let title = format!(
        " {} | {} | {} ",
        session.started_at.format("%Y-%m-%d %H:%M"),
        session.branch,
        commit_short,
    );
    let block = Block::bordered()
        .title(title)
        .title_alignment(Alignment::Center)
        .border_style(Style::new().fg(Color::Cyan));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if detail.runs.is_empty() {
        f.render_widget(
            Paragraph::new("No test runs in this session.").style(STYLE_MUTED),
            inner,
        );
        return;
    }

    let header = Row::new(vec!["Test", "Outcome", "Duration", "Retries"])
        .style(STYLE_BOLD)
        .bottom_margin(1);

    let rows: Vec<Row> = detail
        .runs
        .iter()
        .map(|run| {
            let (label, style) = outcome_label_style(run.outcome);
            Row::new(vec![
                Cell::from(run.test_name.as_ref()),
                Cell::from(label).style(style),
                Cell::from(format!("{:.0}ms", run.duration.as_secs_f64() * 1000.0)),
                Cell::from(run.retry_count.to_string()),
            ])
        })
        .collect();

    let widths = [
        Constraint::Min(30),
        Constraint::Length(8),
        Constraint::Length(10),
        Constraint::Length(8),
    ];

    let summary = format!(
        "{} tests | {} passed | {} failed",
        detail.runs.len(),
        detail
            .runs
            .iter()
            .filter(|r| r.outcome == TestOutcome::Passed)
            .count(),
        detail
            .runs
            .iter()
            .filter(|r| r.outcome != TestOutcome::Passed && r.outcome != TestOutcome::Ignored)
            .count(),
    );

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .split(inner);

    f.render_widget(Paragraph::new(summary).style(STYLE_MUTED), chunks[0]);

    let row_count = rows.len();
    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(STYLE_SELECTED);
    f.render_stateful_widget(table, chunks[1], &mut detail.table_state);

    let mut scrollbar_state =
        ScrollbarState::new(row_count).position(detail.table_state.selected().unwrap_or(0));
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("^"))
            .end_symbol(Some("v")),
        chunks[1],
        &mut scrollbar_state,
    );

    let keys = Line::from(vec![
        Span::styled("j/k", STYLE_BOLD),
        Span::styled(":nav  ", STYLE_MUTED),
        Span::styled("Enter/q/Esc", STYLE_BOLD),
        Span::styled(":back", STYLE_MUTED),
    ]);
    f.render_widget(Paragraph::new(keys), chunks[2]);
}

const fn outcome_label_style(outcome: TestOutcome) -> (&'static str, Style) {
    match outcome {
        TestOutcome::Passed => ("PASS", Style::new().fg(Color::Green)),
        TestOutcome::Failed => ("FAIL", Style::new().fg(Color::Red)),
        TestOutcome::Timeout => ("TIME", Style::new().fg(Color::Yellow)),
        TestOutcome::Panic => (
            "PANC",
            Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        TestOutcome::Ignored => ("SKIP", Style::new().fg(Color::DarkGray)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{test_score, test_session};
    use crate::types::FlakinessScore;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use rstest::rstest;

    #[rstest]
    #[case(Vec::new())]
    #[case(vec![test_score("tests::a", 0.5), test_score("tests::b", 0.01)])]
    fn draw_scores_does_not_panic(#[case] scores: Vec<FlakinessScore>) {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = super::super::app::ScoresApp::new(scores, 0.95);
        terminal.draw(|f| draw_scores(f, &mut app)).unwrap();
    }

    #[test]
    fn draw_scores_with_detail_overlay() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let scores = vec![test_score("tests::flaky", 0.25)];
        let mut app = super::super::app::ScoresApp::new(scores, 0.95);
        let detail = super::super::app::DetailData {
            runs: Vec::new(),
            trend: None,
            patterns: Vec::new(),
        };
        app.enter_detail(detail);
        terminal.draw(|f| draw_scores(f, &mut app)).unwrap();
    }

    #[rstest]
    #[case(Vec::new())]
    #[case(vec![
        test_session("abc123def456", "main"),
        test_session("fedcba987654", "feature/tui"),
    ])]
    fn draw_history_does_not_panic(#[case] sessions: Vec<crate::types::RunSession>) {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = super::super::app::HistoryApp::new(sessions);
        terminal.draw(|f| draw_history(f, &mut app)).unwrap();
    }
}
