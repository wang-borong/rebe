use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::error::{RebeError, RebeResult};
use mdict_rs::MdxFile;
use serde_json::Value;
use sha2::{Digest, Sha256};

const DEFAULT_TIMEOUT_MS: u64 = 10_000;
pub const DEFAULT_DEFINITION_MAX_CHARS: usize = 600;
const YOUDAO_API_URL: &str = "https://openapi.youdao.com/api";
const YOUDAO_APP_KEY_ENV: &str = "YOUDAO_APP_KEY";
const YOUDAO_APP_SECRET_ENV: &str = "YOUDAO_APP_SECRET";
const COPYTRANSLATOR_YOUDAO_APP_KEY_ENV: &str = "VUE_APP_YOUDAO_APP_KEY";
const COPYTRANSLATOR_YOUDAO_APP_SECRET_ENV: &str = "VUE_APP_YOUDAO_APP_SECRET";

pub enum DefinitionProvider {
    Command(DefinitionCommand),
    Youdao(YoudaoDefinitionClient),
    Mdx(MdxDefinitionClient),
}

impl DefinitionProvider {
    pub fn lookup(&mut self, word: &str) -> RebeResult<Option<String>> {
        match self {
            Self::Command(command) => command.lookup(word),
            Self::Youdao(client) => client.lookup(word),
            Self::Mdx(client) => client.lookup(word),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdxDefinitionFormat {
    Plain,
    Html,
}

impl MdxDefinitionFormat {
    pub fn parse(value: &str) -> RebeResult<Self> {
        match value {
            "plain" | "text" => Ok(Self::Plain),
            "html" | "raw" => Ok(Self::Html),
            _ => Err(RebeError::InvalidArgument(format!(
                "unsupported MDX definition format: {value}; expected plain or html"
            ))),
        }
    }
}

pub struct DefinitionCommand {
    template: String,
    timeout: Duration,
    max_chars: Option<usize>,
    cache: HashMap<String, Option<String>>,
}

impl DefinitionCommand {
    pub fn new(template: String, timeout_ms: u64, max_chars: Option<usize>) -> RebeResult<Self> {
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
            max_chars: normalize_max_chars(max_chars),
            cache: HashMap::new(),
        })
    }

    pub fn lookup(&mut self, word: &str) -> RebeResult<Option<String>> {
        if let Some(cached) = self.cache.get(word) {
            return Ok(cached.clone());
        }

        let command = render_command(&self.template, word);
        let result = run_shell_command(&command, self.timeout, self.max_chars)?;
        self.cache.insert(word.to_string(), result.clone());

        Ok(result)
    }
}

#[derive(Debug, Clone)]
pub struct YoudaoDefinitionConfig {
    app_key: String,
    app_secret: String,
    from: String,
    to: String,
    timeout: Duration,
    max_chars: Option<usize>,
}

impl YoudaoDefinitionConfig {
    pub fn from_options(
        app_key: Option<String>,
        app_secret: Option<String>,
        from: Option<String>,
        to: Option<String>,
        timeout_ms: u64,
        max_chars: Option<usize>,
    ) -> RebeResult<Self> {
        let app_key = option_or_env(
            app_key,
            &[YOUDAO_APP_KEY_ENV, COPYTRANSLATOR_YOUDAO_APP_KEY_ENV],
        )
        .ok_or_else(|| {
            RebeError::InvalidArgument(format!(
                "missing Youdao app key; pass --youdao-app-key or set {YOUDAO_APP_KEY_ENV}/{COPYTRANSLATOR_YOUDAO_APP_KEY_ENV}"
            ))
        })?;
        let app_secret = option_or_env(
            app_secret,
            &[
                YOUDAO_APP_SECRET_ENV,
                COPYTRANSLATOR_YOUDAO_APP_SECRET_ENV,
            ],
        )
        .ok_or_else(|| {
            RebeError::InvalidArgument(format!(
                "missing Youdao app secret; pass --youdao-app-secret or set {YOUDAO_APP_SECRET_ENV}/{COPYTRANSLATOR_YOUDAO_APP_SECRET_ENV}"
            ))
        })?;
        let from = from
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "en".to_string());
        let to = to
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "zh-CHS".to_string());
        let timeout_ms = if timeout_ms == 0 {
            DEFAULT_TIMEOUT_MS
        } else {
            timeout_ms
        };

        Ok(Self {
            app_key,
            app_secret,
            from,
            to,
            timeout: Duration::from_millis(timeout_ms),
            max_chars: normalize_max_chars(max_chars),
        })
    }
}

pub struct YoudaoDefinitionClient {
    config: YoudaoDefinitionConfig,
    agent: ureq::Agent,
    cache: HashMap<String, Option<String>>,
}

impl YoudaoDefinitionClient {
    pub fn new(config: YoudaoDefinitionConfig) -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(config.timeout))
            .build()
            .into();

        Self {
            config,
            agent,
            cache: HashMap::new(),
        }
    }

    pub fn lookup(&mut self, word: &str) -> RebeResult<Option<String>> {
        if let Some(cached) = self.cache.get(word) {
            return Ok(cached.clone());
        }

        let result = self.request(word).ok().and_then(|body| {
            parse_youdao_definition(&body, self.config.max_chars)
                .ok()
                .flatten()
        });
        self.cache.insert(word.to_string(), result.clone());

        Ok(result)
    }

    fn request(&self, word: &str) -> Result<String, String> {
        let salt = current_millis().to_string();
        let curtime = current_seconds().to_string();
        let sign = youdao_sign(
            &self.config.app_key,
            word,
            &salt,
            &curtime,
            &self.config.app_secret,
        );
        let form = vec![
            ("q", word.to_string()),
            ("appKey", self.config.app_key.clone()),
            ("salt", salt),
            ("from", self.config.from.clone()),
            ("to", self.config.to.clone()),
            ("sign", sign),
            ("signType", "v3".to_string()),
            ("curtime", curtime),
        ];
        let mut response = self
            .agent
            .post(YOUDAO_API_URL)
            .send_form(form)
            .map_err(|err| err.to_string())?;

        response
            .body_mut()
            .read_to_string()
            .map_err(|err| err.to_string())
    }
}

pub struct MdxDefinitionClient {
    dictionary: MdxFile,
    max_chars: Option<usize>,
    format: MdxDefinitionFormat,
    cache: HashMap<String, Option<String>>,
}

impl MdxDefinitionClient {
    pub fn open(
        path: impl AsRef<Path>,
        max_chars: Option<usize>,
        format: MdxDefinitionFormat,
    ) -> RebeResult<Self> {
        let mdx_path = resolve_mdx_path(path.as_ref())?;
        let dictionary = MdxFile::open(&mdx_path).map_err(|err| {
            RebeError::InvalidArgument(format!(
                "failed to open MDX dictionary {}: {err}",
                mdx_path.display()
            ))
        })?;

        Ok(Self {
            dictionary,
            max_chars: normalize_max_chars(max_chars),
            format,
            cache: HashMap::new(),
        })
    }

    pub fn lookup(&mut self, word: &str) -> RebeResult<Option<String>> {
        if let Some(cached) = self.cache.get(word) {
            return Ok(cached.clone());
        }

        let result = self
            .dictionary
            .lookup(word)
            .map_err(|err| {
                RebeError::InvalidArgument(format!("failed to look up {word} in MDX: {err}"))
            })?
            .and_then(|record| clean_mdx_definition(&record.text, self.max_chars, self.format));
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

fn run_shell_command(
    command: &str,
    timeout: Duration,
    max_chars: Option<usize>,
) -> RebeResult<Option<String>> {
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
            return Ok(clean_definition(&definition, max_chars));
        }

        if started_at.elapsed() >= timeout {
            child.kill().ok();
            child.wait().ok();
            return Ok(None);
        }

        thread::sleep(Duration::from_millis(20));
    }
}

fn clean_definition(raw: &str, max_chars: Option<usize>) -> Option<String> {
    let compact = raw.split_whitespace().collect::<Vec<_>>().join(" ");

    if compact.is_empty() {
        return None;
    }

    match normalize_max_chars(max_chars) {
        Some(max_chars) => Some(trim_chars(&compact, max_chars)),
        None => Some(compact),
    }
}

fn clean_mdx_definition(
    raw: &str,
    max_chars: Option<usize>,
    format: MdxDefinitionFormat,
) -> Option<String> {
    let text = match format {
        MdxDefinitionFormat::Plain if raw.contains('<') && raw.contains('>') => {
            html2text::from_read(raw.as_bytes(), 80).unwrap_or_else(|_| raw.to_string())
        }
        MdxDefinitionFormat::Plain | MdxDefinitionFormat::Html => raw.to_string(),
    };

    clean_definition(&text, max_chars)
}

fn resolve_mdx_path(path: &Path) -> RebeResult<PathBuf> {
    if path.is_file() {
        if has_extension(path, "mdx") {
            return Ok(path.to_path_buf());
        }

        return Err(RebeError::InvalidArgument(format!(
            "--define-mdx expects a .mdx file or dictionary directory, got {}",
            path.display()
        )));
    }

    if !path.is_dir() {
        return Err(RebeError::InvalidArgument(format!(
            "MDX dictionary path does not exist or is not readable: {}",
            path.display()
        )));
    }

    let mut mdx_paths = fs::read_dir(path)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|entry_path| entry_path.is_file() && has_extension(entry_path, "mdx"))
        .collect::<Vec<_>>();
    mdx_paths.sort();

    match mdx_paths.len() {
        0 => Err(RebeError::InvalidArgument(format!(
            "no .mdx file found in dictionary directory: {}",
            path.display()
        ))),
        1 => Ok(mdx_paths.remove(0)),
        _ => Err(RebeError::InvalidArgument(format!(
            "multiple .mdx files found in {}; pass the exact .mdx file path",
            path.display()
        ))),
    }
}

fn has_extension(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.eq_ignore_ascii_case(expected))
        .unwrap_or(false)
}

fn parse_youdao_definition(raw: &str, max_chars: Option<usize>) -> RebeResult<Option<String>> {
    let payload: Value = serde_json::from_str(raw).map_err(|err| {
        RebeError::InvalidArgument(format!("invalid Youdao JSON response: {err}"))
    })?;
    let error_code = payload
        .get("errorCode")
        .and_then(Value::as_str)
        .unwrap_or("0");

    if error_code != "0" {
        return Ok(None);
    }

    let mut parts = Vec::new();

    if let Some(basic) = payload.get("basic") {
        let phonetic_count = parts.len();
        push_youdao_phonetic(&mut parts, basic, "uk-phonetic", "英");
        push_youdao_phonetic(&mut parts, basic, "us-phonetic", "美");

        if parts.len() == phonetic_count {
            push_youdao_phonetic(&mut parts, basic, "phonetic", "音标");
        }

        if let Some(explains) = basic.get("explains").and_then(Value::as_array) {
            parts.extend(
                explains
                    .iter()
                    .filter_map(Value::as_str)
                    .take(4)
                    .map(String::from),
            );
        }
    }

    if parts.is_empty() {
        push_json_string_array(&mut parts, &payload, "translation", 3);
    }

    if let Some(web_items) = payload.get("web").and_then(Value::as_array) {
        for item in web_items.iter().take(2) {
            let key = item.get("key").and_then(Value::as_str).unwrap_or("");
            let Some(values) = item.get("value").and_then(Value::as_array) else {
                continue;
            };
            let translations = values
                .iter()
                .filter_map(Value::as_str)
                .take(3)
                .collect::<Vec<_>>();

            if !translations.is_empty() {
                if key.is_empty() {
                    parts.push(format!("网络: {}", translations.join("/")));
                } else {
                    parts.push(format!("网络 {key}: {}", translations.join("/")));
                }
            }
        }
    }

    Ok(clean_definition(&parts.join("; "), max_chars))
}

fn push_youdao_phonetic(parts: &mut Vec<String>, basic: &Value, field: &str, label: &str) {
    let Some(value) = basic.get(field).and_then(Value::as_str) else {
        return;
    };

    if !value.trim().is_empty() {
        parts.push(format!("{label} [{value}]"));
    }
}

fn push_json_string_array(parts: &mut Vec<String>, payload: &Value, field: &str, limit: usize) {
    let Some(values) = payload.get(field).and_then(Value::as_array) else {
        return;
    };

    parts.extend(
        values
            .iter()
            .filter_map(Value::as_str)
            .take(limit)
            .map(String::from),
    );
}

fn option_or_env(value: Option<String>, env_names: &[&str]) -> Option<String> {
    value.filter(|value| !value.trim().is_empty()).or_else(|| {
        env_names
            .iter()
            .filter_map(|name| env::var(name).ok())
            .find(|value| !value.trim().is_empty())
    })
}

fn youdao_sign(app_key: &str, query: &str, salt: &str, curtime: &str, app_secret: &str) -> String {
    let input = format!(
        "{}{}{}{}{}",
        app_key,
        truncate_youdao_query(query),
        salt,
        curtime,
        app_secret
    );
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn truncate_youdao_query(query: &str) -> String {
    let chars = query.chars().collect::<Vec<_>>();

    if chars.len() <= 20 {
        return query.to_string();
    }

    let start = chars.iter().take(10).collect::<String>();
    let end = chars
        .iter()
        .skip(chars.len().saturating_sub(10))
        .collect::<String>();

    format!("{}{length}{}", start, end, length = chars.len())
}

fn current_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn current_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn normalize_max_chars(max_chars: Option<usize>) -> Option<usize> {
    max_chars.filter(|max_chars| *max_chars > 0)
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
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    const TEST_MDX_BASE64: &str = include_str!("../tests/fixtures/portable-test.mdx.base64");

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
            Some(DEFAULT_DEFINITION_MAX_CHARS),
        )
        .expect("definition command");

        let definition = command.lookup("reader").expect("lookup");
        assert_eq!(definition, Some("definition for reader".to_string()));
    }

    #[test]
    fn external_command_definition_respects_max_chars() {
        let mut command = DefinitionCommand::new(
            "printf '%s' 'abcdef ghijkl' # {word}".to_string(),
            DEFAULT_TIMEOUT_MS,
            Some(8),
        )
        .expect("definition command");

        let definition = command.lookup("reader").expect("lookup");
        assert_eq!(definition, Some("abcdef g...".to_string()));
    }

    #[test]
    fn external_command_definition_can_be_unlimited() {
        let mut command = DefinitionCommand::new(
            "printf '%s' 'abcdef ghijkl' # {word}".to_string(),
            DEFAULT_TIMEOUT_MS,
            None,
        )
        .expect("definition command");

        let definition = command.lookup("reader").expect("lookup");
        assert_eq!(definition, Some("abcdef ghijkl".to_string()));
    }

    #[test]
    fn truncates_youdao_query_like_open_translate() {
        assert_eq!(truncate_youdao_query("short"), "short");
        assert_eq!(
            truncate_youdao_query("abcdefghijklmnopqrstuvwxyz"),
            "abcdefghij26qrstuvwxyz"
        );
    }

    #[test]
    fn signs_youdao_request_like_open_translate() {
        let sign = youdao_sign(
            "appKey",
            "abcdefghijklmnopqrstuvwxyz",
            "12345",
            "67890",
            "secret",
        );

        assert_eq!(
            sign,
            "7a5665069bff27196da8bbbca65f3a5def5fa03ec70044a8908d8dd36588b94d"
        );
    }

    #[test]
    fn parses_youdao_definition_response() {
        let raw = r#"{
            "errorCode": "0",
            "query": "reader",
            "translation": ["读者"],
            "basic": {
                "uk-phonetic": "ˈriːdə(r)",
                "us-phonetic": "ˈriːdər",
                "explains": ["n. 读者；读本", "n. 阅读器"]
            },
            "web": [
                {"key": "Reader", "value": ["读者", "阅读器"]}
            ]
        }"#;

        let definition = parse_youdao_definition(raw, Some(DEFAULT_DEFINITION_MAX_CHARS))
            .expect("parse")
            .expect("definition");

        assert!(definition.contains("英 [ˈriːdə(r)]"));
        assert!(definition.contains("n. 读者；读本"));
        assert!(definition.contains("网络 Reader: 读者/阅读器"));
    }

    #[test]
    fn ignores_youdao_error_response() {
        let definition =
            parse_youdao_definition(r#"{"errorCode":"101"}"#, Some(DEFAULT_DEFINITION_MAX_CHARS))
                .expect("parse");
        assert_eq!(definition, None);
    }

    #[test]
    fn cleans_mdx_html_definition() {
        let definition = clean_mdx_definition(
            r#"<link href="style.css" rel="stylesheet"><div><b>reader</b><span>读者</span></div>"#,
            Some(DEFAULT_DEFINITION_MAX_CHARS),
            MdxDefinitionFormat::Plain,
        )
        .expect("definition");

        assert!(definition.contains("reader"));
        assert!(definition.contains("读者"));
        assert!(!definition.contains("<div>"));
    }

    #[test]
    fn keeps_mdx_html_definition_when_requested() {
        let definition = clean_mdx_definition(
            r#"<div><b>reader</b><span>读者</span></div>"#,
            Some(DEFAULT_DEFINITION_MAX_CHARS),
            MdxDefinitionFormat::Html,
        )
        .expect("definition");

        assert!(definition.contains("<div>"));
        assert!(definition.contains("<b>reader</b>"));
        assert!(definition.contains("读者"));
    }

    #[test]
    fn looks_up_repository_mdx_fixture() {
        let path = temp_file_path("fixture", "mdx");
        let fixture = STANDARD
            .decode(TEST_MDX_BASE64.lines().collect::<String>())
            .expect("decode mdx fixture");
        fs::write(&path, fixture).expect("write mdx fixture");

        let mut client = MdxDefinitionClient::open(
            &path,
            Some(DEFAULT_DEFINITION_MAX_CHARS),
            MdxDefinitionFormat::Plain,
        )
        .expect("open mdx");
        let definition = client.lookup("hello").expect("lookup").expect("definition");

        assert!(definition.contains("hello"));
        assert!(definition.contains("greeting"));
        assert!(!definition.contains("<b>"));

        fs::remove_file(path).ok();
    }

    fn temp_file_path(name: &str, extension: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();

        env::temp_dir().join(format!("rebe_dict_{name}_{nanos}.{extension}"))
    }
}
