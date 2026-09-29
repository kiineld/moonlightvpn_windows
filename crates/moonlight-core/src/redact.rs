//! Masks what must never reach the screen from the log: the subscription link,
//! its host and token, and every server address.
//!
//! The log is a screen like any other, and both halves of it leak: the app's
//! own narration quotes errors that carry the request URL, and mihomo names the
//! server it dials on every connection line. One shared set of secrets is
//! applied to both before a line leaves the controller, so nothing downstream
//! has to remember to.

use std::sync::{Arc, RwLock};

use regex::{Regex, RegexBuilder};

#[derive(Clone, Default)]
pub struct Redactions(Arc<RwLock<Option<Regex>>>);

impl Redactions {
    /// Replaces the set. Strings under four characters are dropped: masking
    /// "ru" or a one-digit port would shred every line for nothing.
    pub fn set(&self, secrets: impl IntoIterator<Item = String>) {
        let mut cleaned: Vec<String> = secrets
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| s.chars().count() >= 4)
            .collect();
        // Longest first, so a link is masked whole before its host is.
        cleaned.sort_by_key(|s| std::cmp::Reverse(s.len()));
        cleaned.dedup();
        let pattern = cleaned
            .iter()
            .map(|s| regex::escape(s))
            .collect::<Vec<_>>()
            .join("|");
        let compiled = (!cleaned.is_empty())
            .then(|| {
                RegexBuilder::new(&pattern)
                    .case_insensitive(true)
                    .build()
                    .ok()
            })
            .flatten();
        *self.0.write().expect("redactions lock") = compiled;
    }

    pub fn apply(&self, line: &str) -> String {
        match self.0.read().expect("redactions lock").as_ref() {
            Some(pattern) => pattern.replace_all(line, "•••").into_owned(),
            None => line.to_string(),
        }
    }
}

/// Everything about a subscription that identifies it: the link as typed and
/// as normalised, its host, the long path segments (the token), and each
/// server the subscription names.
pub fn secrets(link: Option<&str>, panel_yaml: Option<&str>) -> Vec<String> {
    let mut secrets = Vec::new();
    if let Some(link) = link {
        secrets.push(link.to_string());
        if let Some(normalised) = crate::subscription::normalize(link) {
            if let Ok(url) = url::Url::parse(&normalised) {
                if let Some(host) = url.host_str() {
                    secrets.push(host.to_string());
                }
                secrets.extend(
                    url.path_segments()
                        .into_iter()
                        .flatten()
                        .filter(|s| s.len() >= 8)
                        .map(str::to_string),
                );
            }
            secrets.push(normalised);
        }
    }
    if let Some(yaml) = panel_yaml {
        if let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(yaml) {
            if let Some(proxies) = value.get("proxies").and_then(|p| p.as_sequence()) {
                secrets.extend(
                    proxies
                        .iter()
                        .filter_map(|p| p.get("server")?.as_str())
                        .map(str::to_string),
                );
            }
        }
    }
    secrets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_link_its_host_token_and_servers_are_masked_and_nothing_else() {
        let redactions = Redactions::default();
        redactions.set(secrets(
            Some("https://sub.example.net/api/sub/AbCdEf123456"),
            Some("proxies:\n  - {name: A, server: node1.example.org, port: 443}\n"),
        ));
        let line = redactions.apply(
            "GET https://sub.example.net/api/sub/AbCdEf123456/mihomo failed; \
             SUB.EXAMPLE.NET token abcdef123456 dial node1.example.org:443 via A",
        );
        assert!(!line.to_lowercase().contains("example"), "{line}");
        assert!(!line.to_lowercase().contains("abcdef123456"), "{line}");
        assert!(line.contains("via A"), "short names survive: {line}");
    }

    #[test]
    fn with_nothing_set_a_line_passes_untouched() {
        assert_eq!(Redactions::default().apply("Core is up"), "Core is up");
    }
}
