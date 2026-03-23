use ratatui::prelude::*;
use ratatui::widgets::{Block, Cell, Clear, Paragraph, Row, Table};

use crate::types::{FlakinessCategory, FlakinessScore, TestOutcome};

use super::app::{AppMode, DetailData, HistoryApp, ScoresApp};

const STYLE_BOLD: Style = Style::new().add_modifier(Modifier::BOLD);
const STYLE_MUTED: Style = Style::new().fg(Color::DarkGray);
const STYLE_SELECTED: Style = Style::new()
    .bg(Color::DarkGray)
    .add_modifier(Modifier::BOLD);

pub fn draw_scores(f: &mut Frame, app: &ScoresApp) {
    let chunks = Layout::vertical([
        Constraint::Length(1),
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
    let text = format!(
        "cargo ninety-nine | {}/{} tests",
        app.filtered.len(),
        app.scores.len()
    );
    f.render_widget(Paragraph::new(text).style(STYLE_BOLD), area);
}

fn draw_filter_bar(f: &mut Frame, app: &ScoresApp, area: Rect) {
    let direction = if app.sort_ascending { "asc" } else { "desc" };
    let text = format!(
        "Filter: {} | Sort: {} ({})",
        app.filter_label(),
        app.sort_field.label(),
        direction
    );
    f.render_widget(Paragraph::new(text).style(STYLE_MUTED), area);
}

fn draw_scores_table(f: &mut Frame, app: &ScoresApp, area: Rect) {
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

    let cursor = app.cursor.position();
    let threshold = app.confidence_threshold;
    let rows: Vec<Row> = app
        .filtered
        .iter()
        .enumerate()
        .map(|(i, &idx)| {
            let s = &app.scores[idx];
            let effective = s.effective_score(threshold);
            let cat = FlakinessCategory::from_score(effective);

            let row = Row::new(vec![
                Cell::from(s.test_name.as_ref()),
                Cell::from(s.total_runs.to_string()),
                Cell::from(format!("{:.1}%", s.pass_rate * 100.0)),
                Cell::from(format!("{effective:.3}")),
                Cell::from(cat.label()).style(category_style(cat)),
                Cell::from(format!("{:.2}", s.confidence)),
            ]);

            highlight_row(row, i == cursor)
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

    let table = Table::new(rows, widths).header(header);
    f.render_widget(table, area);
}

fn draw_scores_footer(f: &mut Frame, area: Rect) {
    f.render_widget(
        Paragraph::new("j/k:nav  s:sort  r:reverse  f:filter  Enter:detail  q:quit")
            .style(STYLE_MUTED),
        area,
    );
}

fn highlight_row(row: Row<'_>, is_selected: bool) -> Row<'_> {
    if is_selected {
        row.style(STYLE_SELECTED)
    } else {
        row
    }
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
        .title_alignment(Alignment::Center);
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
            let symbol = match run.outcome {
                TestOutcome::Passed => "PASS",
                TestOutcome::Failed => "FAIL",
                TestOutcome::Ignored => "SKIP",
                TestOutcome::Timeout => "TIME",
                TestOutcome::Panic => "PANC",
            };
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

pub fn draw_history(f: &mut Frame, app: &HistoryApp) {
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .split(f.area());

    let header_text = format!("cargo ninety-nine | {} sessions", app.sessions.len());
    f.render_widget(Paragraph::new(header_text).style(STYLE_BOLD), chunks[0]);

    let header = Row::new(vec!["Date", "Tests", "Flaky", "Branch", "Commit"])
        .style(STYLE_BOLD)
        .bottom_margin(1);

    let cursor = app.cursor.position();
    let rows: Vec<Row> = app
        .sessions
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let row = Row::new(vec![
                Cell::from(s.started_at.format("%Y-%m-%d %H:%M").to_string()),
                Cell::from(s.test_count.to_string()),
                Cell::from(s.flaky_count.to_string()),
                Cell::from(s.branch.as_str()),
                Cell::from(s.commit_hash.get(..8).unwrap_or(&s.commit_hash)),
            ]);

            highlight_row(row, i == cursor)
        })
        .collect();

    let widths = [
        Constraint::Length(18),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Min(15),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, widths).header(header);
    f.render_widget(table, chunks[1]);

    f.render_widget(
        Paragraph::new("j/k:nav  q:quit").style(STYLE_MUTED),
        chunks[2],
    );
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
        let app = super::super::app::ScoresApp::new(scores, 0.95);
        terminal.draw(|f| draw_scores(f, &app)).unwrap();
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
        terminal.draw(|f| draw_scores(f, &app)).unwrap();
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
        let app = super::super::app::HistoryApp::new(sessions);
        terminal.draw(|f| draw_history(f, &app)).unwrap();
    }
}
