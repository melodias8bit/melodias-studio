//! Static export of the dashboard overview from `melodias history --csv`.
//!
//! It reads the runs CSV (plus the optional findings and trend CSVs) and
//! renders the same overview the live dashboard shows: totals, the score
//! trend, the findings that cost the most, the outcome mix and the per-run
//! table. The output is one self-contained `index.html`: inline CSS, inline
//! SVG, no server, no database, no JavaScript and no external assets.
//!
//! The CSV carries no cost, timing, candidate, patch or verdict data, so the
//! spend, time, candidates, patches and editorial axes panels of the live
//! dashboard are marked as unavailable instead of guessed.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::history;

/// One row of `melodias history --csv` (the runs view).
#[derive(Debug, Clone)]
pub struct Run {
    pub id: i64,
    pub started_at: String,
    pub slug: String,
    pub technical: Option<f64>,
    pub editorial: Option<f64>,
    pub total: Option<f64>,
    pub accepted: bool,
    pub outcome: Option<String>,
    pub outcome_reason: Option<String>,
    pub rounds: i64,
}

/// One row of `melodias history findings --csv`.
#[derive(Debug, Clone)]
pub struct Finding {
    pub code: String,
    pub hits: i64,
    pub lost: Option<f64>,
    pub points: Option<f64>,
    pub accepted: i64,
}

/// One row of `melodias history trend --csv`.
#[derive(Debug, Clone)]
pub struct Trend {
    pub bucket: String,
    pub runs: i64,
    pub technical: Option<f64>,
    pub total: Option<f64>,
    pub accepted: i64,
}

/// The overview totals, computed the same way as the dashboard summary: the
/// accepted count sums the accepted flag, the limit count sums the
/// `accepted_by_limit` outcomes, below is the remainder, and the means skip
/// missing scores.
#[derive(Debug, Default)]
pub struct Totals {
    pub runs: i64,
    pub accepted: i64,
    pub by_limit: i64,
    pub below: i64,
    pub technical: Option<f64>,
    pub editorial: Option<f64>,
    pub total: Option<f64>,
    pub best_id: Option<i64>,
    pub best_slug: String,
    pub best_total: Option<f64>,
    pub latest_id: Option<i64>,
    pub latest_slug: String,
    pub latest_at: String,
    pub findings: i64,
    pub lost_points: f64,
}

/// Human names for the deterministic finding codes, kept in sync with
/// `FINDING_LABELS` in `dashboard/app.js`.
fn finding_label(code: &str) -> &str {
    match code {
        "IMG_PALETTE" => "Color palette",
        "IMG_CONTRAST" => "Contrast",
        "IMG_SATURATION" => "Saturation",
        "IMG_GLOW" => "Glowing eyes",
        "IMG_TEXT" => "Text or watermark",
        "AUD_HIGHS" => "High frequencies",
        "AUD_BUILD" => "Intro build",
        "AUD_DURATION" => "Track length",
        "AUD_SILENCE" => "Silence gaps",
        "AUD_LOUDNESS" => "Loudness",
        "AUD_PEAK" => "True peak",
        "AUD_CENTROID" => "Timbre brightness",
        "AUD_REPEAT" => "Repeating pattern",
        "AUD_VARIETY" => "Pattern variety",
        "GATE_TYPE" => "Rejected: size or aspect",
        "GATE_BLANK" => "Rejected: blank frame",
        "GATE_TEXT" => "Rejected: text on image",
        "GATE_DURATION" => "Rejected: wrong length",
        "GATE_PEAK" => "Rejected: clipping",
        "GATE_LEADING" => "Rejected: leading silence",
        _ => code,
    }
}

pub fn summarize(runs: &[Run], findings: &[Finding]) -> Totals {
    let mut totals = Totals::default();
    totals.runs = runs.len() as i64;
    totals.accepted = runs.iter().filter(|run| run.accepted).count() as i64;
    totals.by_limit = runs
        .iter()
        .filter(|run| run.outcome.as_deref() == Some("accepted_by_limit"))
        .count() as i64;
    totals.below = (totals.runs - totals.accepted - totals.by_limit).max(0);
    totals.technical = mean(runs.iter().filter_map(|run| run.technical));
    totals.editorial = mean(runs.iter().filter_map(|run| run.editorial));
    totals.total = mean(runs.iter().filter_map(|run| run.total));
    let mut best: Option<&Run> = None;
    for run in runs {
        if run.total.is_none() {
            continue;
        }
        if best.is_none_or(|top| run.total > top.total) {
            best = Some(run);
        }
    }
    if let Some(run) = best {
        totals.best_id = Some(run.id);
        totals.best_slug = run.slug.clone();
        totals.best_total = run.total;
    }
    if let Some(run) = runs.iter().max_by_key(|run| run.id) {
        totals.latest_id = Some(run.id);
        totals.latest_slug = run.slug.clone();
        totals.latest_at = run.started_at.clone();
    }
    totals.findings = findings.iter().map(|row| row.hits).sum();
    totals.lost_points = findings
        .iter()
        .filter_map(|row| row.points)
        .filter(|points| *points < 0.0)
        .sum();
    // A missing sum reads as zero, never as negative zero.
    if totals.lost_points == 0.0 {
        totals.lost_points = 0.0;
    }
    totals
}

fn mean(values: impl Iterator<Item = f64>) -> Option<f64> {
    let mut sum = 0.0;
    let mut count = 0_i64;
    for value in values {
        sum += value;
        count += 1;
    }
    (count > 0).then_some(sum / count as f64)
}

/// Splits one CSV document into rows. It understands the quoting that
/// `history::render_*` writes: a field with a comma, quote or newline is
/// wrapped in quotes and each inner quote is doubled.
fn parse_csv(text: &str) -> Result<Vec<Vec<String>>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    while let Some(char) = chars.next() {
        if in_quotes {
            if char == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(char);
            }
            continue;
        }
        match char {
            '"' if field.is_empty() && !quoted => {
                quoted = true;
                in_quotes = true;
            }
            ',' => {
                row.push(std::mem::take(&mut field));
                quoted = false;
            }
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(std::mem::take(&mut field));
                quoted = false;
                push_row(&mut rows, &mut row);
            }
            '\n' => {
                row.push(std::mem::take(&mut field));
                quoted = false;
                push_row(&mut rows, &mut row);
            }
            _ => field.push(char),
        }
    }
    if in_quotes {
        bail!("unterminated quoted field in the CSV");
    }
    if !field.is_empty() || quoted || !row.is_empty() {
        row.push(field);
        push_row(&mut rows, &mut row);
    }
    Ok(rows)
}

fn push_row(rows: &mut Vec<Vec<String>>, row: &mut Vec<String>) {
    if row.len() == 1 && row[0].is_empty() {
        row.clear();
        return;
    }
    rows.push(std::mem::take(row));
}

fn header(rows: &[Vec<String>], expected: &[&str], what: &str) -> Result<()> {
    let first = rows.first();
    let matches = first.is_some_and(|head| {
        head.len() == expected.len() && head.iter().zip(expected).all(|(got, want)| got == want)
    });
    if !matches {
        bail!("expected the {what} CSV header: {}", expected.join(","));
    }
    Ok(())
}

fn number(value: &str) -> Result<Option<f64>> {
    let value = value.trim();
    if value.is_empty() || value == "-" {
        return Ok(None);
    }
    value
        .parse::<f64>()
        .map(Some)
        .with_context(|| format!("invalid number in the CSV: {value}"))
}

fn integer(value: &str, column: &str) -> Result<i64> {
    value.trim().parse::<i64>().with_context(|| {
        format!(
            "invalid integer in the CSV column {column}: {}",
            value.trim()
        )
    })
}

pub fn parse_runs_csv(text: &str) -> Result<Vec<Run>> {
    let rows = parse_csv(text)?;
    if rows.is_empty() {
        bail!("the runs CSV is empty");
    }
    header(
        &rows,
        &[
            "id",
            "started_at",
            "slug",
            "technical",
            "editorial",
            "total",
            "accepted",
            "outcome",
            "outcome_reason",
            "rounds",
        ],
        "runs",
    )?;
    let mut runs = Vec::with_capacity(rows.len().saturating_sub(1));
    for row in &rows[1..] {
        if row.len() != 10 {
            bail!("expected 10 columns in the runs CSV, got {}", row.len());
        }
        let accepted = match row[6].trim() {
            "true" | "1" => true,
            "false" | "0" => false,
            other => bail!("invalid accepted flag in the runs CSV: {other}"),
        };
        let outcome = row[7].trim().to_string();
        let reason = row[8].trim().to_string();
        runs.push(Run {
            id: integer(&row[0], "id")?,
            started_at: row[1].clone(),
            slug: row[2].clone(),
            technical: number(&row[3])?,
            editorial: number(&row[4])?,
            total: number(&row[5])?,
            accepted,
            outcome: (!outcome.is_empty()).then_some(outcome),
            outcome_reason: (!reason.is_empty()).then_some(reason),
            rounds: integer(&row[9], "rounds")?,
        });
    }
    Ok(runs)
}

pub fn parse_findings_csv(text: &str) -> Result<Vec<Finding>> {
    let rows = parse_csv(text)?;
    if rows.is_empty() {
        bail!("the findings CSV is empty");
    }
    header(
        &rows,
        &["code", "hits", "lost", "points", "accepted"],
        "findings",
    )?;
    let mut out = Vec::with_capacity(rows.len().saturating_sub(1));
    for row in &rows[1..] {
        if row.len() != 5 {
            bail!("expected 5 columns in the findings CSV, got {}", row.len());
        }
        out.push(Finding {
            code: row[0].clone(),
            hits: integer(&row[1], "hits")?,
            lost: number(&row[2])?,
            points: number(&row[3])?,
            accepted: integer(&row[4], "accepted")?,
        });
    }
    Ok(out)
}

pub fn parse_trend_csv(text: &str) -> Result<Vec<Trend>> {
    let rows = parse_csv(text)?;
    if rows.is_empty() {
        bail!("the trend CSV is empty");
    }
    header(
        &rows,
        &["bucket", "runs", "technical", "total", "accepted"],
        "trend",
    )?;
    let mut out = Vec::with_capacity(rows.len().saturating_sub(1));
    for row in &rows[1..] {
        if row.len() != 5 {
            bail!("expected 5 columns in the trend CSV, got {}", row.len());
        }
        out.push(Trend {
            bucket: row[0].clone(),
            runs: integer(&row[1], "runs")?,
            technical: number(&row[2])?,
            total: number(&row[3])?,
            accepted: integer(&row[4], "accepted")?,
        });
    }
    Ok(out)
}

fn outcome_label(run: &Run) -> &str {
    match run.outcome.as_deref() {
        Some("accepted") => "accepted",
        Some("accepted_by_limit") => "by limit",
        Some("below_target") => "below",
        Some("failed") => "failed",
        Some(_) => "below",
        None if run.accepted => "accepted",
        None => "below",
    }
}

fn outcome_kind(run: &Run) -> &'static str {
    match run.outcome.as_deref() {
        Some("accepted") => "ok",
        Some("accepted_by_limit") => "limit",
        Some("below_target") => "low",
        Some("failed") => "low",
        Some(_) => "low",
        None if run.accepted => "ok",
        None => "low",
    }
}

fn fmt(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.1}"))
        .unwrap_or_else(|| "-".to_string())
}

fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for char in value.chars() {
        match char {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(char),
        }
    }
    out
}

/// Renders the whole page: totals, score trend, findings, outcomes, the trend
/// when one was given, and the per-run table.
pub fn render(
    runs: &[Run],
    findings: &[Finding],
    trend: &[Trend],
    sources: &str,
    generated_at: &str,
) -> String {
    let totals = summarize(runs, findings);
    let mut ordered: Vec<&Run> = runs.iter().collect();
    ordered.sort_by_key(|run| run.id);
    let mut page = String::new();
    let _ = writeln!(
        page,
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\" />\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />\n\
         <title>melodias export</title>\n<style>\n{CSS}\n</style>\n</head>\n<body>\n\
         <main class=\"page\">\n<header class=\"top\">\n<h1>melodias export</h1>\n\
         <p class=\"hint\">static overview of {} run{} \u{00b7} generated {}</p>\n\
         <p class=\"hint\">source: {}</p>\n</header>\n",
        totals.runs,
        if totals.runs == 1 { "" } else { "s" },
        escape(generated_at),
        escape(sources),
    );
    render_cards(&mut page, &totals);
    if runs.is_empty() {
        page.push_str(
            "<section class=\"panel\"><p><strong>No runs yet.</strong> \
             The CSV holds no run rows.</p></section>\n",
        );
    } else {
        render_score(&mut page, &ordered);
        render_findings(&mut page, findings);
        render_outcomes(&mut page, &totals);
        if !trend.is_empty() {
            render_trend(&mut page, trend);
        }
        render_runs(&mut page, &ordered);
    }
    page.push_str(
        "<footer class=\"foot\"><p>Judge spend, run time, candidate and patch \
         counts and the editorial axes need the live dashboard: the history \
         CSV carries no cost, timing or verdict data.</p></footer>\n\
         </main>\n</body>\n</html>\n",
    );
    page
}

fn render_cards(page: &mut String, totals: &Totals) {
    let cards: &[(&str, String, &str)] = &[
        ("Runs", totals.runs.to_string(), ""),
        (
            "Accepted",
            format!("{}/{}", totals.accepted, totals.runs),
            "",
        ),
        ("By limit", totals.by_limit.to_string(), ""),
        ("Below", totals.below.to_string(), ""),
        ("Best total", fmt(totals.best_total), &totals.best_slug),
        ("Mean total", fmt(totals.total), "target 80"),
        ("Mean technical", fmt(totals.technical), "target 85"),
        ("Mean editorial", fmt(totals.editorial), "target 70"),
        ("Findings", totals.findings.to_string(), ""),
        (
            "Lost points",
            format!("{:.1}", totals.lost_points.abs()),
            "points deducted",
        ),
        ("Last run", latest_label(totals), &totals.latest_slug),
    ];
    page.push_str("<section class=\"cards\" aria-label=\"Totals\">");
    for (label, value, hint) in cards {
        let _ = writeln!(
            page,
            "<div class=\"card\"><div class=\"label\">{}</div>\
             <div class=\"value\">{}</div>",
            escape(label),
            escape(value),
        );
        if !hint.is_empty() {
            let _ = writeln!(
                page,
                "<div class=\"hint\" title=\"{}\">{}</div>",
                escape(hint),
                escape(hint),
            );
        }
        page.push_str("</div>");
    }
    page.push_str("</section>");
}

fn latest_label(totals: &Totals) -> String {
    match totals.latest_id {
        Some(id) => format!("#{id}"),
        None => "-".to_string(),
    }
}

fn render_score(page: &mut String, runs: &[&Run]) {
    page.push_str(
        "<section class=\"panel\"><div class=\"panel-head\"><h2>Score over runs</h2>\
         <div class=\"legend\"><span><i class=\"swatch tech\"></i>technical</span> \
         <span><i class=\"swatch edit\"></i>editorial</span> \
         <span><i class=\"swatch total\"></i>total</span></div></div>\n",
    );
    page.push_str(&score_svg(runs));
    page.push_str("</section>");
}

/// The same geometry as the live chart: 900 by 320, the 0 to 100 scale with
/// gridlines every 25, the target line at 80, and one series per score.
fn score_svg(runs: &[&Run]) -> String {
    let width = 900.0;
    let height = 320.0;
    let left = 46.0;
    let right = 18.0;
    let top = 18.0;
    let bottom = 38.0;
    let plot_width = width - left - right;
    let plot_height = height - top - bottom;
    let x = |index: usize| {
        if runs.len() <= 1 {
            left + plot_width / 2.0
        } else {
            left + (index as f64 / (runs.len() - 1) as f64) * plot_width
        }
    };
    let y = |value: f64| top + plot_height - (value.clamp(0.0, 100.0) / 100.0) * plot_height;
    let mut svg = format!(
        "<svg class=\"chart\" viewBox=\"0 0 {width} {height}\" role=\"img\" \
         aria-label=\"Technical, editorial and total score per run\">\n",
    );
    for mark in [0.0, 25.0, 50.0, 75.0, 100.0] {
        let _ = writeln!(
            svg,
            "<line x1=\"{left}\" x2=\"{r}\" y1=\"{y:.1}\" y2=\"{y:.1}\" class=\"grid\" />\n\
             <text x=\"{lx}\" y=\"{ty:.1}\" class=\"axis\" text-anchor=\"end\">{mark}</text>\n",
            r = left + plot_width,
            y = y(mark),
            lx = left - 8.0,
            ty = y(mark) + 4.0,
        );
    }
    let _ = writeln!(
        svg,
        "<line x1=\"{left}\" x2=\"{r}\" y1=\"{y:.1}\" y2=\"{y:.1}\" class=\"target\" />\n\
         <text x=\"{lx}\" y=\"{ty:.1}\" class=\"target-label\">target 80</text>\n",
        r = left + plot_width,
        y = y(80.0),
        lx = left + 6.0,
        ty = y(80.0) - 6.0,
    );
    for (class, name, pick) in [
        (
            "tech",
            "technical",
            (|run: &Run| run.technical) as fn(&Run) -> Option<f64>,
        ),
        (
            "edit",
            "editorial",
            (|run: &Run| run.editorial) as fn(&Run) -> Option<f64>,
        ),
        (
            "total",
            "total",
            (|run: &Run| run.total) as fn(&Run) -> Option<f64>,
        ),
    ] {
        let values: Vec<Option<f64>> = runs.iter().map(|run| pick(run)).collect();
        let mut path = String::new();
        let mut pen = false;
        for (index, value) in values.iter().enumerate() {
            let Some(value) = value else {
                pen = false;
                continue;
            };
            path.push_str(&format!(
                "{}{:.1},{:.1} ",
                if pen { 'L' } else { 'M' },
                x(index),
                y(*value)
            ));
            pen = true;
        }
        if !path.is_empty() {
            let _ = writeln!(
                svg,
                "<path d=\"{}\" class=\"line {class}\" />\n",
                path.trim_end()
            );
        }
        for (index, value) in values.iter().enumerate() {
            let Some(value) = value else { continue };
            let run = runs[index];
            let _ = writeln!(
                svg,
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"3.5\" class=\"dot {class}\">\
                 <title>#{id} \u{00b7} {name} {score} \u{00b7} {at}</title></circle>\n",
                x(index),
                y(*value),
                id = run.id,
                score = fmt(Some(*value)),
                at = escape(&run.started_at),
            );
        }
    }
    let step = runs.len().div_ceil(8).max(1);
    for (index, run) in runs.iter().enumerate() {
        if index % step != 0 && index + 1 != runs.len() {
            continue;
        }
        let _ = writeln!(
            svg,
            "<text x=\"{:.1}\" y=\"{:.1}\" class=\"axis\" text-anchor=\"middle\">#{}</text>\n",
            x(index),
            height - 14.0,
            run.id,
        );
    }
    svg.push_str("</svg>");
    svg
}

fn render_findings(page: &mut String, findings: &[Finding]) {
    page.push_str(
        "<section class=\"panel\"><div class=\"panel-head\"><h2>What costs the most</h2>\
         <p class=\"hint\">lost points per finding</p></div>\n",
    );
    let mut rows: Vec<&Finding> = findings
        .iter()
        .filter(|row| row.points.is_some_and(|points| points < 0.0))
        .collect();
    rows.sort_by(|a, b| {
        a.points
            .partial_cmp(&b.points)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows.truncate(12);
    if rows.is_empty() {
        page.push_str("<p class=\"note\">No lost points.</p></section>");
    } else {
        page.push_str(&findings_svg(&rows));
    }
    page.push_str(
        "<div class=\"table-wrap\"><table><thead><tr><th>Finding</th><th class=\"num\">Hits</th>\
         <th class=\"num\">Avg</th><th class=\"num\">Points</th><th class=\"num\">Accepted</th>\
         </tr></thead><tbody>\n",
    );
    let mut sorted: Vec<&Finding> = findings.iter().collect();
    sorted.sort_by(|a, b| {
        a.lost
            .unwrap_or(0.0)
            .partial_cmp(&b.lost.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for row in sorted {
        let _ = writeln!(
            page,
            "<tr><td><div>{}</div><div class=\"code-hint\">{}</div></td>\
             <td class=\"num\">{}</td><td class=\"num\">{}</td>\
             <td class=\"num\">{}</td><td class=\"num\">{}</td></tr>\n",
            escape(finding_label(&row.code)),
            escape(&row.code),
            row.hits,
            fmt(row.lost),
            fmt(row.points),
            row.accepted,
        );
    }
    page.push_str("</tbody></table></div></section>");
}

fn findings_svg(rows: &[&Finding]) -> String {
    let width = 900.0;
    let label_width = 180.0;
    let left = label_width + 12.0;
    let value_width = 120.0;
    let bar_area = width - left - value_width;
    let height = 20.0 + rows.len() as f64 * 30.0;
    let longest = rows
        .iter()
        .map(|row| row.points.unwrap_or(0.0).abs())
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let mut svg = format!(
        "<svg class=\"chart\" viewBox=\"0 0 {width} {height}\" role=\"img\" \
         aria-label=\"Lost points per finding\">\n",
    );
    for (index, row) in rows.iter().enumerate() {
        let lost = row.points.unwrap_or(0.0);
        let top = 20.0 + index as f64 * 30.0;
        let bar = (lost.abs() / longest * bar_area).max(3.0);
        let _ = writeln!(
            svg,
            "<text x=\"{label_width}\" y=\"{:.1}\" class=\"row-label\" text-anchor=\"end\">{}<title>{}</title></text>\n\
             <rect x=\"{left}\" y=\"{top:.1}\" width=\"{bar:.1}\" height=\"16\" rx=\"3\" class=\"bar\" />\n\
             <text x=\"{:.1}\" y=\"{:.1}\" class=\"row-value\">{:.1} \u{00b7} {}\u{d7}</text>\n",
            top + 13.0,
            escape(finding_label(&row.code)),
            escape(&row.code),
            left + bar + 8.0,
            top + 13.0,
            lost,
            row.hits,
        );
    }
    svg.push_str("</svg>");
    svg
}

fn render_outcomes(page: &mut String, totals: &Totals) {
    page.push_str(
        "<section class=\"panel\"><div class=\"panel-head\"><h2>Outcomes</h2>\
         <p class=\"hint\">accepted, by limit and below target</p></div>\n",
    );
    let width = 420.0;
    let mut svg = format!(
        "<svg class=\"chart outcomes\" viewBox=\"0 0 {width} 70\" role=\"img\" \
         aria-label=\"Runs per outcome\">\n",
    );
    let segments = [
        ("Accepted", totals.accepted, "ok"),
        ("By limit", totals.by_limit, "limit"),
        ("Below target", totals.below, "low"),
    ];
    let mut x = 0.0;
    for (_, count, kind) in &segments {
        if *count == 0 || totals.runs == 0 {
            continue;
        }
        let bar = *count as f64 / totals.runs as f64 * width;
        let _ = writeln!(
            svg,
            "<rect x=\"{x:.1}\" y=\"8\" width=\"{bar:.1}\" height=\"24\" rx=\"3\" \
             class=\"outcome {kind}\"><title>{label} {count}</title></rect>\n",
            label = segments
                .iter()
                .find(|(_, _, k)| k == kind)
                .map(|(label, _, _)| *label)
                .unwrap_or(""),
        );
        x += bar;
    }
    if totals.runs == 0 {
        svg.push_str(
            "<text x=\"210\" y=\"28\" class=\"note\" text-anchor=\"middle\">No runs yet.</text>\n",
        );
    }
    svg.push_str("</svg>\n<div class=\"outcome-legend\">");
    for (label, count, kind) in &segments {
        let _ = writeln!(
            svg,
            "<span><i class=\"swatch {kind}\"></i>{label} {count}</span>",
        );
    }
    svg.push_str("</div></section>");
    page.push_str(&svg);
}

fn render_trend(page: &mut String, trend: &[Trend]) {
    page.push_str(
        "<section class=\"panel\"><div class=\"panel-head\"><h2>Trend</h2>\
         <p class=\"hint\">average technical and total per bucket</p></div>\n\
         <div class=\"table-wrap\"><table><thead><tr><th>Bucket</th>\
         <th class=\"num\">Runs</th><th class=\"num\">Tech</th><th class=\"num\">Total</th>\
         <th class=\"num\">Accepted</th></tr></thead><tbody>\n",
    );
    for row in trend {
        let _ = writeln!(
            page,
            "<tr><td class=\"mono\">{}</td><td class=\"num\">{}</td>\
             <td class=\"num\">{}</td><td class=\"num\">{}</td>\
             <td class=\"num\">{}</td></tr>\n",
            escape(&row.bucket),
            row.runs,
            fmt(row.technical),
            fmt(row.total),
            row.accepted,
        );
    }
    page.push_str("</tbody></table></div></section>");
}

fn render_runs(page: &mut String, runs: &[&Run]) {
    page.push_str(
        "<section class=\"panel\"><div class=\"panel-head\"><h2>Runs</h2>\
         <p class=\"hint\">newest last, as in the score trend</p></div>\n\
         <div class=\"table-wrap\"><table><thead><tr><th class=\"num\">#</th>\
         <th>When</th><th>Slug</th><th class=\"num\">Tech</th><th class=\"num\">Edit</th>\
         <th class=\"num\">Total</th><th>Rounds</th><th>Status</th></tr></thead><tbody>\n",
    );
    let mut descending: Vec<&Run> = runs.to_vec();
    descending.sort_by_key(|run| std::cmp::Reverse(run.id));
    for run in descending {
        let reason = run.outcome_reason.as_deref().unwrap_or("");
        let _ = writeln!(
            page,
            "<tr><td class=\"num\">#{}</td><td class=\"mono\">{}</td>\
             <td class=\"slug\">{}</td><td class=\"num\">{}</td>\
             <td class=\"num\">{}</td><td class=\"num\">{}</td>\
             <td class=\"num\">{}</td>\
             <td><span class=\"badge {}\"{}>{}</span></td></tr>\n",
            run.id,
            escape(&run.started_at),
            escape(&run.slug),
            fmt(run.technical),
            fmt(run.editorial),
            fmt(run.total),
            run.rounds,
            outcome_kind(run),
            if reason.is_empty() {
                String::new()
            } else {
                format!(" title=\"{}\"", escape(reason))
            },
            outcome_label(run),
        );
    }
    page.push_str("</tbody></table></div></section>");
}

/// Writes `index.html` into `out_dir` from the three CSV documents.
pub fn export(
    runs_csv: &str,
    findings_csv: Option<&str>,
    trend_csv: Option<&str>,
) -> Result<String> {
    let runs = parse_runs_csv(runs_csv)?;
    let findings = findings_csv
        .map(parse_findings_csv)
        .transpose()?
        .unwrap_or_default();
    let trend = trend_csv
        .map(parse_trend_csv)
        .transpose()?
        .unwrap_or_default();
    let mut sources = String::from("runs.csv");
    if findings_csv.is_some() {
        sources.push_str(", findings.csv");
    }
    if trend_csv.is_some() {
        sources.push_str(", trend.csv");
    }
    Ok(render(
        &runs,
        &findings,
        &trend,
        &sources,
        &history::now_iso8601(),
    ))
}

/// Reads the CSV files, renders the page and writes `index.html`.
pub fn export_to_dir(
    runs_path: &Path,
    findings_path: Option<&Path>,
    trend_path: Option<&Path>,
    out_dir: &Path,
) -> Result<()> {
    let runs_csv = fs::read_to_string(runs_path)
        .with_context(|| format!("reading {}", runs_path.display()))?;
    let findings_csv = findings_path
        .map(fs::read_to_string)
        .transpose()
        .with_context(|| {
            format!(
                "reading {}",
                findings_path
                    .map(|path| path.display().to_string())
                    .unwrap_or_default()
            )
        })?;
    let trend_csv = trend_path
        .map(fs::read_to_string)
        .transpose()
        .with_context(|| {
            format!(
                "reading {}",
                trend_path
                    .map(|path| path.display().to_string())
                    .unwrap_or_default()
            )
        })?;
    let page = export(&runs_csv, findings_csv.as_deref(), trend_csv.as_deref())?;
    fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;
    fs::write(out_dir.join("index.html"), page)
        .with_context(|| format!("writing {}", out_dir.join("index.html").display()))?;
    Ok(())
}

const CSS: &str = "
:root {
  color-scheme: dark light;
  --bg: #0d0f13;
  --panel: #151922;
  --ink: #e7eaf1;
  --muted: #8d96a8;
  --line: #262d3a;
  --grid: #202633;
  --accent: #5eead4;
  --tech: #60a5fa;
  --edit: #c084fc;
  --total: #34d399;
  --good: #34d399;
  --bad: #f87171;
  --warn: #f59e0b;
}
@media (prefers-color-scheme: light) {
  :root {
    --bg: #f4f6fa;
    --panel: #ffffff;
    --ink: #161b25;
    --muted: #667085;
    --line: #dde3ee;
    --grid: #e8edf6;
    --accent: #0f9e8a;
    --tech: #2563eb;
    --edit: #7c3aed;
    --total: #059669;
    --good: #059669;
    --bad: #dc2626;
    --warn: #b45309;
  }
}
* { box-sizing: border-box; }
body {
  margin: 0;
  background: var(--bg);
  color: var(--ink);
  font-family: system-ui, -apple-system, \"Segoe UI\", Roboto, sans-serif;
  font-size: 15px;
  line-height: 1.45;
}
.page { max-width: 1100px; margin: 0 auto; padding: 24px 18px 48px; }
.top h1 { margin: 0; font-size: 24px; }
.hint { color: var(--muted); font-size: 13px; margin: 4px 0; }
.cards {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 10px;
  margin: 18px 0;
}
@media (max-width: 900px) { .cards { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
.card { border: 1px solid var(--line); border-radius: 10px; padding: 10px 12px; background: var(--panel); }
.card .label { font-size: 11px; text-transform: uppercase; letter-spacing: 0.08em; color: var(--muted); }
.card .value { font-size: 22px; font-weight: 650; }
.card .hint { font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.panel { border: 1px solid var(--line); border-radius: 12px; padding: 14px 18px 18px; margin: 14px 0; background: var(--panel); }
.panel-head { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; flex-wrap: wrap; }
.panel-head h2 { margin: 6px 0; font-size: 17px; }
.legend { display: flex; gap: 14px; font-size: 12px; color: var(--muted); }
.legend span { display: inline-flex; align-items: center; gap: 6px; }
.swatch { width: 10px; height: 10px; border-radius: 3px; display: inline-block; }
.swatch.tech { background: var(--tech); }
.swatch.edit { background: var(--edit); }
.swatch.total { background: var(--total); }
.swatch.ok { background: var(--good); }
.swatch.limit { background: var(--warn); }
.swatch.low { background: var(--bad); }
.chart { display: block; width: 100%; height: auto; }
.chart .grid { stroke: var(--grid); stroke-width: 1; }
.chart .axis { fill: var(--muted); font-size: 11px; font-family: ui-monospace, Menlo, Consolas, monospace; }
.chart .target { stroke: var(--muted); stroke-width: 1; stroke-dasharray: 4 5; opacity: 0.7; }
.chart .target-label { fill: var(--muted); font-size: 10px; letter-spacing: 0.04em; text-transform: uppercase; }
.chart .line { fill: none; stroke-width: 2; stroke-linejoin: round; stroke-linecap: round; }
.chart .line.tech, .chart .dot.tech { stroke: var(--tech); }
.chart .line.edit, .chart .dot.edit { stroke: var(--edit); }
.chart .line.total, .chart .dot.total { stroke: var(--total); }
.chart .dot { fill: var(--panel); stroke-width: 2; }
.chart .row-label { fill: var(--ink); font-size: 12px; font-family: ui-monospace, Menlo, Consolas, monospace; }
.chart .row-value { fill: var(--muted); font-size: 11px; font-family: ui-monospace, Menlo, Consolas, monospace; }
.chart .bar { fill: var(--accent); opacity: 0.8; }
.chart .outcome.ok { fill: var(--good); }
.chart .outcome.limit { fill: var(--warn); }
.chart .outcome.low { fill: var(--bad); }
.chart .note { fill: var(--muted); font-size: 13px; }
.chart.outcomes { max-width: 420px; }
.outcome-legend { display: flex; flex-wrap: wrap; gap: 6px 14px; font-size: 12px; color: var(--muted); margin-top: 6px; }
.outcome-legend span { display: inline-flex; align-items: center; gap: 6px; }
.table-wrap { overflow-x: auto; }
table { width: 100%; border-collapse: collapse; font-size: 13px; }
thead th {
  text-align: left; font-size: 11px; text-transform: uppercase; letter-spacing: 0.08em;
  color: var(--muted); font-weight: 550; padding: 6px 10px; border-bottom: 1px solid var(--line);
  white-space: nowrap;
}
thead th.num { text-align: right; }
tbody td { padding: 6px 10px; border-bottom: 1px solid var(--line); vertical-align: top; }
tbody td.num { text-align: right; font-variant-numeric: tabular-nums; }
.mono, .code-hint { font-family: ui-monospace, Menlo, Consolas, monospace; }
.code-hint { font-size: 11px; color: var(--muted); }
.slug { font-weight: 550; }
.badge {
  display: inline-block; font-size: 12px; padding: 1px 8px; border-radius: 999px;
  border: 1px solid var(--line); color: var(--muted); white-space: nowrap;
}
.badge.ok { color: var(--good); border-color: var(--good); }
.badge.limit { color: var(--warn); border-color: var(--warn); }
.badge.low { color: var(--bad); border-color: var(--bad); }
.note { color: var(--muted); }
.foot { color: var(--muted); font-size: 12px; margin-top: 18px; }
";

#[cfg(test)]
#[path = "tests/export.rs"]
mod tests;
