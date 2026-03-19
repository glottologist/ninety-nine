use std::fmt::Write as FmtWrite;
use std::io::Write;
use std::path::Path;

use crate::error::NinetyNineError;
use crate::types::{FlakinessCategory, FlakinessScore};

/// Exports scores as `JUnit` XML.
///
/// # Errors
///
/// Returns `Io` if the file cannot be written.
pub fn export_junit(scores: &[FlakinessScore], path: &Path) -> Result<(), NinetyNineError> {
    let mut xml = String::with_capacity(4096);
    writeln!(xml, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>").ok();
    writeln!(
        xml,
        "<testsuites name=\"flaky-test-detection\" tests=\"{}\" failures=\"{}\">",
        scores.len(),
        scores
            .iter()
            .filter(|s| s.probability_flaky >= 0.05)
            .count(),
    )
    .ok();
    writeln!(
        xml,
        "  <testsuite name=\"flakiness\" tests=\"{}\">",
        scores.len()
    )
    .ok();

    for score in scores {
        let name = xml_escape(&score.test_name);
        let category = FlakinessCategory::from_score(score.probability_flaky);

        if score.probability_flaky >= 0.05 {
            writeln!(xml, "    <testcase name=\"{name}\" time=\"0\">").ok();
            writeln!(
                xml,
                "      <failure message=\"flaky: {:.1}% probability, category: {category}\">",
                score.probability_flaky * 100.0,
            )
            .ok();
            writeln!(
                xml,
                "pass_rate={:.1}% total_runs={} consecutive_failures={}",
                score.pass_rate * 100.0,
                score.total_runs,
                score.consecutive_failures,
            )
            .ok();
            writeln!(xml, "      </failure>").ok();
            writeln!(xml, "    </testcase>").ok();
        } else {
            writeln!(xml, "    <testcase name=\"{name}\" time=\"0\" />").ok();
        }
    }

    writeln!(xml, "  </testsuite>").ok();
    writeln!(xml, "</testsuites>").ok();

    write_file(path, xml.as_bytes())
}

/// Exports scores as CSV.
///
/// # Errors
///
/// Returns `Io` if the file cannot be written.
pub fn export_csv(scores: &[FlakinessScore], path: &Path) -> Result<(), NinetyNineError> {
    let mut out = String::with_capacity(2048);
    writeln!(
        out,
        "test_name,probability_flaky,pass_rate,total_runs,consecutive_failures,category,confidence"
    )
    .ok();

    for score in scores {
        let category = FlakinessCategory::from_score(score.probability_flaky);
        writeln!(
            out,
            "{},{:.6},{:.6},{},{},{},{:.6}",
            csv_escape(&score.test_name),
            score.probability_flaky,
            score.pass_rate,
            score.total_runs,
            score.consecutive_failures,
            category,
            score.confidence,
        )
        .ok();
    }

    write_file(path, out.as_bytes())
}

/// Exports scores as HTML.
///
/// # Errors
///
/// Returns `Io` if the file cannot be written.
pub fn export_html(scores: &[FlakinessScore], path: &Path) -> Result<(), NinetyNineError> {
    let mut html = String::with_capacity(8192);
    write_html_head(&mut html);

    let flaky_count = scores
        .iter()
        .filter(|s| s.probability_flaky >= 0.05)
        .count();
    write_html_summary(&mut html, scores.len(), flaky_count);
    write_html_table_header(&mut html);

    for score in scores {
        write_html_table_row(&mut html, score);
    }

    write_html_footer(&mut html);
    write_file(path, html.as_bytes())
}

fn write_html_head(buf: &mut String) {
    writeln!(buf, "<!DOCTYPE html>").ok();
    writeln!(buf, "<html lang=\"en\"><head>").ok();
    writeln!(buf, "<meta charset=\"UTF-8\">").ok();
    writeln!(buf, "<title>Flaky Test Report — cargo ninety-nine</title>").ok();
    writeln!(buf, "<style>").ok();
    writeln!(
        buf,
        "body {{ font-family: system-ui, sans-serif; margin: 2rem; background: #fafafa; }}"
    )
    .ok();
    writeln!(
        buf,
        "h1 {{ color: #333; }} table {{ border-collapse: collapse; width: 100%; }}"
    )
    .ok();
    writeln!(
        buf,
        "th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}"
    )
    .ok();
    writeln!(buf, "th {{ background: #4a90d9; color: white; }}").ok();
    writeln!(buf, "tr:nth-child(even) {{ background: #f2f2f2; }}").ok();
    writeln!(
        buf,
        ".stable {{ color: #27ae60; }} .occasional {{ color: #f39c12; }}"
    )
    .ok();
    writeln!(
        buf,
        ".moderate {{ color: #e74c3c; }} .frequent {{ color: #c0392b; font-weight: bold; }}"
    )
    .ok();
    writeln!(
        buf,
        ".critical {{ color: white; background: #c0392b; padding: 2px 6px; border-radius: 3px; }}"
    )
    .ok();
    writeln!(buf, "</style></head><body>").ok();
    writeln!(buf, "<h1>Flaky Test Detection Report</h1>").ok();
}

fn write_html_summary(buf: &mut String, total: usize, flaky_count: usize) {
    writeln!(
        buf,
        "<p><strong>{total}</strong> tests analyzed, <strong>{flaky_count}</strong> flagged as flaky.</p>",
    )
    .ok();
}

fn write_html_table_header(buf: &mut String) {
    writeln!(buf, "<table><thead><tr>").ok();
    writeln!(
        buf,
        "<th>Test</th><th>P(flaky)</th><th>Pass Rate</th><th>Runs</th><th>Consec. Fails</th><th>Category</th>"
    )
    .ok();
    writeln!(buf, "</tr></thead><tbody>").ok();
}

fn write_html_table_row(buf: &mut String, score: &FlakinessScore) {
    let category = FlakinessCategory::from_score(score.probability_flaky);
    let css_class = category.label().to_lowercase();
    let name = html_escape(&score.test_name);

    writeln!(buf, "<tr>").ok();
    writeln!(buf, "  <td><code>{name}</code></td>").ok();
    writeln!(buf, "  <td>{:.1}%</td>", score.probability_flaky * 100.0).ok();
    writeln!(buf, "  <td>{:.1}%</td>", score.pass_rate * 100.0).ok();
    writeln!(buf, "  <td>{}</td>", score.total_runs).ok();
    writeln!(buf, "  <td>{}</td>", score.consecutive_failures).ok();
    writeln!(
        buf,
        "  <td><span class=\"{css_class}\">{category}</span></td>"
    )
    .ok();
    writeln!(buf, "</tr>").ok();
}

fn write_html_footer(buf: &mut String) {
    writeln!(buf, "</tbody></table>").ok();
    writeln!(
        buf,
        "<footer><p>Generated by <em>cargo ninety-nine</em></p></footer>"
    )
    .ok();
    writeln!(buf, "</body></html>").ok();
}

/// Exports scores as JSON.
///
/// # Errors
///
/// Returns `Json` if serialization fails, or `Io` if the file cannot be written.
pub fn export_json(scores: &[FlakinessScore], path: &Path) -> Result<(), NinetyNineError> {
    let json = serde_json::to_string_pretty(scores)?;
    write_file(path, json.as_bytes())
}

fn write_file(path: &Path, data: &[u8]) -> Result<(), NinetyNineError> {
    let mut file = std::fs::File::create(path)?;
    file.write_all(data)?;
    Ok(())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tempfile::NamedTempFile;

    fn sample_score(name: &str, probability: f64) -> FlakinessScore {
        use crate::types::{BayesianParams, TestName};
        FlakinessScore {
            test_name: TestName::from(name),
            probability_flaky: probability,
            confidence: 0.95,
            pass_rate: 1.0 - probability,
            fail_rate: probability,
            total_runs: 10,
            consecutive_failures: if probability > 0.1 { 2 } else { 0 },
            last_updated: chrono::Utc::now(),
            bayesian_params: BayesianParams {
                alpha: 1.0,
                beta: 1.0,
                posterior_mean: probability,
                posterior_variance: 0.01,
                credible_interval_lower: 0.0,
                credible_interval_upper: 1.0,
            },
        }
    }

    #[test]
    fn junit_export_writes_valid_xml() {
        let scores = vec![
            sample_score("test::stable", 0.01),
            sample_score("test::flaky", 0.25),
        ];
        let tmp = NamedTempFile::new().unwrap();
        export_junit(&scores, tmp.path()).unwrap();
        let content = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(content.contains("<?xml"));
        assert!(content.contains("test::stable"));
        assert!(content.contains("<failure"));
    }

    #[test]
    fn csv_export_has_header_and_rows() {
        let scores = vec![sample_score("test::one", 0.1)];
        let tmp = NamedTempFile::new().unwrap();
        export_csv(&scores, tmp.path()).unwrap();
        let content = std::fs::read_to_string(tmp.path()).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("test_name,"));
        assert!(lines[1].starts_with("test::one,"));
    }

    #[test]
    fn html_export_contains_structure() {
        let scores = vec![sample_score("test::html", 0.5)];
        let tmp = NamedTempFile::new().unwrap();
        export_html(&scores, tmp.path()).unwrap();
        let content = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(content.contains("<!DOCTYPE html>"));
        assert!(content.contains("test::html"));
        assert!(content.contains("<table>"));
    }

    #[test]
    fn json_export_is_valid_json() {
        let scores = vec![sample_score("test::json", 0.3)];
        let tmp = NamedTempFile::new().unwrap();
        export_json(&scores, tmp.path()).unwrap();
        let content = std::fs::read_to_string(tmp.path()).unwrap();
        let parsed: Vec<FlakinessScore> = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].test_name, "test::json");
    }

    proptest! {
        #[test]
        fn xml_escape_never_contains_raw_special_chars(s in ".*") {
            let escaped = xml_escape(&s);
            let without_entities = escaped
                .replace("&amp;", "")
                .replace("&lt;", "")
                .replace("&gt;", "")
                .replace("&quot;", "")
                .replace("&apos;", "");
            prop_assert!(!without_entities.contains('&'));
            prop_assert!(!without_entities.contains('<'));
            prop_assert!(!without_entities.contains('>'));
        }

        #[test]
        fn html_escape_never_contains_raw_special_chars(s in ".*") {
            let escaped = html_escape(&s);
            let without_entities = escaped
                .replace("&amp;", "")
                .replace("&lt;", "")
                .replace("&gt;", "")
                .replace("&quot;", "");
            prop_assert!(!without_entities.contains('&'));
            prop_assert!(!without_entities.contains('<'));
            prop_assert!(!without_entities.contains('>'));
        }

        #[test]
        fn csv_escape_roundtrip_preserves_content(s in "[a-zA-Z0-9_:, \"\\n]{0,100}") {
            let escaped = csv_escape(&s);
            if escaped.starts_with('"') {
                let inner = &escaped[1..escaped.len()-1];
                let unescaped = inner.replace("\"\"", "\"");
                prop_assert_eq!(s, unescaped);
            } else {
                prop_assert_eq!(&s, &escaped);
            }
        }

        #[test]
        fn junit_export_always_succeeds(
            name in "[a-zA-Z_:]{1,50}",
            prob in 0.0f64..=1.0,
        ) {
            let scores = vec![sample_score(&name, prob)];
            let tmp = NamedTempFile::new().unwrap();
            prop_assert!(export_junit(&scores, tmp.path()).is_ok());
        }
    }
}
