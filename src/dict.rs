use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{RebeError, RebeResult};

const DEFAULT_TIMEOUT_MS: u64 = 10_000;
const MAX_DEFINITION_CHARS: usize = 600;

pub struct DefinitionCommand {
    template: String,
    timeout: Duration,
    cache: HashMap<String, Option<String>>,
}

impl DefinitionCommand {
    pub fn new(template: String, timeout_ms: u64) -> RebeResult<Self> {
        if !template.contains("{word}")
            && !template.contains("{word_raw}")
            && !template.contains("{word_url}")
        {
            return Err(RebeError::InvalidArgument(
                "--define-command must contain {word}, {word_raw}, or {word_url}".to_string(),
            ));
        }

        let timeout_ms = if timeout_ms == 0 {
            DEFAULT_TIMEOUT_MS
        } else {
            timeout_ms
        };

        Ok(Self {
            template,
            timeout: Duration::from_millis(timeout_ms),
            cache: HashMap::new(),
        })
    }

    pub fn lookup(&mut self, word: &str) -> RebeResult<Option<String>> {
        if let Some(cached) = self.cache.get(word) {
            return Ok(cached.clone());
        }

        let command = render_command(&self.template, word);
        let result = run_shell_command(&command, self.timeout)?;
        self.cache.insert(word.to_string(), result.clone());

        Ok(result)
    }
}

pub fn render_command(template: &str, word: &str) -> String {
    template
        .replace("{word_url}", &url_encode_word(word))
        .replace("{word_raw}", word)
        .replace("{word}", &shell_quote(word))
}

fn run_shell_command(command: &str, timeout: Duration) -> RebeResult<Option<String>> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let started_at = Instant::now();

    loop {
        if let Some(status) = child.try_wait()? {
            if !status.success() {
                return Ok(None);
            }

            let output = child.wait_with_output()?;
            let definition = String::from_utf8_lossy(&output.stdout);
            return Ok(clean_definition(&definition));
        }

        if started_at.elapsed() >= timeout {
            child.kill().ok();
            child.wait().ok();
            return Ok(None);
        }

        thread::sleep(Duration::from_millis(20));
    }
}

fn clean_definition(raw: &str) -> Option<String> {
    let compact = raw.split_whitespace().collect::<Vec<_>>().join(" ");

    if compact.is_empty() {
        return None;
    }

    Some(trim_chars(&compact, MAX_DEFINITION_CHARS))
}

fn trim_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let trimmed = chars.by_ref().take(max_chars).collect::<String>();

    if chars.next().is_some() {
        format!("{}...", trimmed.trim_end())
    } else {
        trimmed
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn url_encode_word(value: &str) -> String {
    let mut encoded = String::new();

    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }

    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_command_with_quoted_word() {
        let command = render_command("printf meaning:%s {word}", "reader");
        assert_eq!(command, "printf meaning:%s 'reader'");
    }

    #[test]
    fn renders_url_encoded_word() {
        let command = render_command("curl https://example.test?q={word_url}", "don't");
        assert_eq!(command, "curl https://example.test?q=don%27t");
    }

    #[test]
    fn lookup_uses_external_command() {
        let mut command = DefinitionCommand::new(
            "printf 'definition for %s' {word}".to_string(),
            DEFAULT_TIMEOUT_MS,
        )
        .expect("definition command");

        let definition = command.lookup("reader").expect("lookup");
        assert_eq!(definition, Some("definition for reader".to_string()));
    }
}
