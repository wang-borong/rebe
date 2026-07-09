use std::fmt::Write;

use crate::error::{RebeError, RebeResult};
use crate::AnalysisReport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Txt,
    Csv,
    Json,
}

impl OutputFormat {
    pub fn parse(value: &str) -> RebeResult<Self> {
        match value {
            "txt" | "text" => Ok(Self::Txt),
            "csv" => Ok(Self::Csv),
            "json" => Ok(Self::Json),
            _ => Err(RebeError::InvalidArgument(format!(
                "unsupported output format: {value}; expected txt, csv, or json"
            ))),
        }
    }
}

pub fn render(report: &AnalysisReport, format: OutputFormat) -> String {
    match format {
        OutputFormat::Txt => render_txt(report),
        OutputFormat::Csv => render_csv(report),
        OutputFormat::Json => render_json(report),
    }
}

fn render_txt(report: &AnalysisReport) -> String {
    let mut output = String::new();

    writeln!(output, "Input: {}", report.input.display()).expect("write string");
    writeln!(output, "Total words: {}", report.total_words).expect("write string");
    writeln!(output, "Unique words: {}", report.unique_words).expect("write string");
    writeln!(output, "Candidate words: {}", report.candidate_words).expect("write string");
    writeln!(output, "Known words loaded: {}", report.known_words).expect("write string");
    writeln!(output, "Ignored words loaded: {}", report.ignored_words).expect("write string");
    writeln!(output).expect("write string");
    writeln!(
        output,
        "word\tcount\tfrequency\tcoverage\tfirst_position\tsentences\tforms\texamples"
    )
    .expect("write string");

    for word in &report.words {
        writeln!(
            output,
            "{}\t{}\t{:.6}\t{:.6}\t{}\t{}\t{}\t{}",
            word.word,
            word.count,
            word.frequency,
            word.cumulative_coverage,
            word.first_position,
            word.sentence_count,
            word.forms.join("|"),
            word.examples.join(" | ")
        )
        .expect("write string");
    }

    output
}

fn render_csv(report: &AnalysisReport) -> String {
    let mut output = String::new();

    output.push_str(
        "word,count,frequency,cumulative_coverage,first_position,sentence_count,forms,examples\n",
    );

    for word in &report.words {
        let row = [
            word.word.clone(),
            word.count.to_string(),
            format!("{:.6}", word.frequency),
            format!("{:.6}", word.cumulative_coverage),
            word.first_position.to_string(),
            word.sentence_count.to_string(),
            word.forms.join("|"),
            word.examples.join(" | "),
        ];

        output.push_str(
            &row.iter()
                .map(|field| csv_escape(field))
                .collect::<Vec<_>>()
                .join(","),
        );
        output.push('\n');
    }

    output
}

fn render_json(report: &AnalysisReport) -> String {
    let mut output = String::new();

    output.push_str("{\n");
    writeln!(
        output,
        "  \"input\": \"{}\",",
        json_escape(&report.input.display().to_string())
    )
    .expect("write string");
    writeln!(output, "  \"total_words\": {},", report.total_words).expect("write string");
    writeln!(output, "  \"unique_words\": {},", report.unique_words).expect("write string");
    writeln!(output, "  \"candidate_words\": {},", report.candidate_words).expect("write string");
    writeln!(output, "  \"known_words\": {},", report.known_words).expect("write string");
    writeln!(output, "  \"ignored_words\": {},", report.ignored_words).expect("write string");
    output.push_str("  \"words\": [\n");

    for (index, word) in report.words.iter().enumerate() {
        output.push_str("    {\n");
        writeln!(output, "      \"word\": \"{}\",", json_escape(&word.word)).expect("write string");
        writeln!(output, "      \"count\": {},", word.count).expect("write string");
        writeln!(output, "      \"frequency\": {:.6},", word.frequency).expect("write string");
        writeln!(
            output,
            "      \"cumulative_coverage\": {:.6},",
            word.cumulative_coverage
        )
        .expect("write string");
        writeln!(output, "      \"first_position\": {},", word.first_position)
            .expect("write string");
        writeln!(output, "      \"sentence_count\": {},", word.sentence_count)
            .expect("write string");
        writeln!(
            output,
            "      \"forms\": {},",
            json_string_array(&word.forms)
        )
        .expect("write string");
        writeln!(
            output,
            "      \"examples\": {}",
            json_string_array(&word.examples)
        )
        .expect("write string");

        if index + 1 == report.words.len() {
            output.push_str("    }\n");
        } else {
            output.push_str("    },\n");
        }
    }

    output.push_str("  ]\n");
    output.push_str("}\n");
    output
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn json_string_array(values: &[String]) -> String {
    let values = values
        .iter()
        .map(|value| format!("\"{}\"", json_escape(value)))
        .collect::<Vec<_>>()
        .join(", ");

    format!("[{values}]")
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::new();

    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => {
                write!(escaped, "\\u{:04x}", ch as u32).expect("write string");
            }
            _ => escaped.push(ch),
        }
    }

    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AnalysisReport, WordStat};
    use std::path::PathBuf;

    #[test]
    fn renders_csv_with_escaped_examples() {
        let report = sample_report();
        let csv = render(&report, OutputFormat::Csv);

        assert!(csv.contains("word,count,frequency"));
        assert!(csv.contains("\"A sentence, with comma.\""));
    }

    #[test]
    fn renders_json_with_words() {
        let report = sample_report();
        let json = render(&report, OutputFormat::Json);

        assert!(json.contains("\"word\": \"read\""));
        assert!(json.contains("\"examples\": [\"A sentence, with comma.\"]"));
    }

    fn sample_report() -> AnalysisReport {
        AnalysisReport {
            input: PathBuf::from("book.txt"),
            total_words: 10,
            unique_words: 4,
            candidate_words: 1,
            ignored_words: 0,
            known_words: 0,
            words: vec![WordStat {
                word: "read".to_string(),
                forms: vec!["reading".to_string()],
                count: 2,
                frequency: 0.2,
                cumulative_coverage: 0.2,
                first_position: 1,
                sentence_count: 1,
                examples: vec!["A sentence, with comma.".to_string()],
            }],
        }
    }
}
