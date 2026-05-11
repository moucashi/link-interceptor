use crate::{
    models::{CandidateKind, Config, OpenCandidate},
    rules::{normalize_protocol_scheme, normalize_root_domain},
    windows_integration,
};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlParts {
    pub scheme: String,
    pub domain: Option<String>,
}

pub fn parse_url_parts(input: &str) -> UrlParts {
    if let Ok(url) = Url::parse(input) {
        let scheme = url.scheme().to_ascii_lowercase();
        let domain = if matches!(scheme.as_str(), "http" | "https") {
            url.domain().map(|domain| domain.to_ascii_lowercase())
        } else {
            None
        };
        return UrlParts { scheme, domain };
    }

    let scheme = input
        .split_once(':')
        .map(|(scheme, _)| scheme.to_ascii_lowercase())
        .unwrap_or_default();
    UrlParts {
        scheme,
        domain: None,
    }
}

pub fn build_candidates(config: &Config, url: &str) -> Vec<OpenCandidate> {
    let parts = parse_url_parts(url);
    let is_web = matches!(parts.scheme.as_str(), "http" | "https");
    let mut candidates = Vec::new();

    for browser in windows_integration::discover_browsers() {
        candidates.push(OpenCandidate::new(
            browser.name,
            CandidateKind::Browser,
            browser.command,
            "%1",
            "已安装的浏览器",
        ));
    }

    if !parts.scheme.is_empty() {
        if let Some(handler) = windows_integration::discover_protocol_handler(&parts.scheme) {
            candidates.push(OpenCandidate::new(
                handler.name,
                CandidateKind::ProtocolHandler,
                handler.command,
                "%1",
                format!("已注册的 {} 协议处理程序", parts.scheme),
            ));
        }
    }

    for rule in matching_domain_rules(config, parts.domain.as_deref()) {
        if let Some(app) = config
            .custom_apps
            .iter()
            .find(|app| app.name.eq_ignore_ascii_case(&rule.app_name))
        {
            candidates.push(OpenCandidate::new(
                app.name.clone(),
                CandidateKind::DomainApp,
                Some(app.executable.clone()),
                app.args_template.clone(),
                format!("域名规则 {}", rule.pattern),
            ));
        }
    }

    for rule in matching_protocol_rules(config, &parts.scheme) {
        if let Some(app) = config
            .custom_apps
            .iter()
            .find(|app| app.name.eq_ignore_ascii_case(&rule.app_name))
        {
            candidates.push(OpenCandidate::new(
                app.name.clone(),
                CandidateKind::ProtocolApp,
                Some(app.executable.clone()),
                app.args_template.clone(),
                format!("协议规则 {}", rule.scheme),
            ));
        }
    }

    if is_web {
        if let Some(domain) = parts.domain.as_deref() {
            candidates.extend(windows_integration::discover_app_uri_handlers(domain));
        }
    }

    candidates.push(windows_integration::shell_fallback_candidate());
    sort_candidates(&mut candidates, is_web);
    deduplicate(candidates)
}

fn sort_candidates(candidates: &mut [OpenCandidate], is_web: bool) {
    candidates.sort_by_key(|candidate| {
        let rank = match candidate.kind {
            CandidateKind::DomainApp if is_web => 0,
            CandidateKind::ProtocolHandler if !is_web => 0,
            CandidateKind::ProtocolApp if !is_web => 1,
            CandidateKind::Browser if is_web => 2,
            CandidateKind::Browser => 3,
            CandidateKind::ProtocolHandler => 4,
            CandidateKind::ProtocolApp => 5,
            CandidateKind::DomainApp => 6,
            CandidateKind::ShellFallback => 7,
        };
        (
            rank,
            !candidate.available,
            candidate.name.to_ascii_lowercase(),
        )
    });
}

fn matching_domain_rules<'a>(
    config: &'a Config,
    domain: Option<&str>,
) -> Vec<&'a crate::models::DomainRule> {
    let Some(domain) = domain else {
        return Vec::new();
    };
    config
        .domain_rules
        .iter()
        .filter(|rule| domain_matches(&rule.pattern, domain))
        .collect()
}

fn matching_protocol_rules<'a>(
    config: &'a Config,
    scheme: &str,
) -> Vec<&'a crate::models::ProtocolRule> {
    let scheme = normalize_protocol_scheme(scheme).unwrap_or_default();
    if scheme.is_empty() {
        return Vec::new();
    }
    config
        .protocol_rules
        .iter()
        .filter(|rule| {
            normalize_protocol_scheme(&rule.scheme)
                .is_some_and(|rule_scheme| rule_scheme.eq_ignore_ascii_case(&scheme))
        })
        .collect()
}

pub fn domain_matches(pattern: &str, domain: &str) -> bool {
    normalize_root_domain(pattern).is_some_and(|pattern| {
        normalize_root_domain(domain).is_some_and(|domain| pattern.eq_ignore_ascii_case(&domain))
    })
}

fn deduplicate(candidates: Vec<OpenCandidate>) -> Vec<OpenCandidate> {
    let mut result = Vec::new();
    for candidate in candidates {
        let duplicate = result.iter().any(|existing: &OpenCandidate| {
            existing.name.eq_ignore_ascii_case(&candidate.name)
                && existing.kind == candidate.kind
                && existing.command == candidate.command
        });
        if !duplicate {
            result.push(candidate);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CustomApp, DomainRule, ProtocolRule};

    #[test]
    fn parses_web_url() {
        let parts = parse_url_parts("https://sub.example.com/a");
        assert_eq!(parts.scheme, "https");
        assert_eq!(parts.domain.as_deref(), Some("sub.example.com"));
    }

    #[test]
    fn parses_custom_scheme() {
        let parts = parse_url_parts("slack://channel?id=1");
        assert_eq!(parts.scheme, "slack");
        assert_eq!(parts.domain, None);
    }

    #[test]
    fn root_domain_matches_ignore_subdomains() {
        assert!(domain_matches("example.com", "a.example.com"));
        assert!(domain_matches("api.example.com", "www.example.com"));
        assert!(domain_matches("*.example.com", "example.com"));
        assert!(!domain_matches("*.example.com", "badexample.com"));
    }

    #[test]
    fn matching_domain_rule_adds_domain_app_candidate() {
        let config = Config {
            custom_apps: vec![CustomApp {
                name: "Example App".to_owned(),
                executable: "example.exe".to_owned(),
                args_template: "{url}".to_owned(),
            }],
            domain_rules: vec![DomainRule {
                pattern: "example.com".to_owned(),
                app_name: "Example App".to_owned(),
            }],
            ..Default::default()
        };
        let candidates = build_candidates(&config, "https://example.com");
        let app = candidates
            .iter()
            .find(|candidate| candidate.kind == CandidateKind::DomainApp)
            .unwrap();
        assert_eq!(app.name, "Example App");
    }

    #[test]
    fn matching_protocol_rule_adds_protocol_app_candidate() {
        let config = Config {
            custom_apps: vec![CustomApp {
                name: "Mail App".to_owned(),
                executable: "mail.exe".to_owned(),
                args_template: "{url}".to_owned(),
            }],
            protocol_rules: vec![ProtocolRule {
                scheme: "mailto".to_owned(),
                app_name: "Mail App".to_owned(),
            }],
            ..Default::default()
        };
        let candidates = build_candidates(&config, "mailto:test@example.com");
        let app = candidates
            .iter()
            .find(|candidate| candidate.kind == CandidateKind::ProtocolApp)
            .unwrap();
        assert_eq!(app.name, "Mail App");
    }

    #[test]
    fn custom_apps_are_not_added_without_matching_rule() {
        let config = Config {
            custom_apps: vec![CustomApp {
                name: "Example App".to_owned(),
                executable: "example.exe".to_owned(),
                args_template: "{url}".to_owned(),
            }],
            ..Default::default()
        };
        let candidates = build_candidates(&config, "https://example.com");
        assert!(!candidates
            .iter()
            .any(|candidate| candidate.name == "Example App"));
    }
}
