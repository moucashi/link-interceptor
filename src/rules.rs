use url::Url;

pub fn normalize_root_domain(input: &str) -> Option<String> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    let domain = Url::parse(input)
        .ok()
        .and_then(|url| url.domain().map(ToOwned::to_owned))
        .unwrap_or_else(|| input.to_owned());
    let domain = domain
        .split('/')
        .next()
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches('.')
        .trim_start_matches("*.")
        .to_ascii_lowercase();
    let mut labels = domain
        .split('.')
        .filter(|label| !label.is_empty())
        .collect::<Vec<_>>();
    if labels.len() < 2 {
        return None;
    }

    let last = labels.pop()?;
    let second_last = labels.pop()?;
    Some(format!("{second_last}.{last}"))
}

pub fn normalize_protocol_scheme(input: &str) -> Option<String> {
    let scheme = input.trim().trim_end_matches(':').to_ascii_lowercase();
    if scheme.is_empty()
        || !scheme
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'))
    {
        return None;
    }

    Some(scheme)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_root_domain() {
        assert_eq!(
            normalize_root_domain("https://api.example.com/path").as_deref(),
            Some("example.com")
        );
        assert_eq!(
            normalize_root_domain("*.Sub.Example.COM.").as_deref(),
            Some("example.com")
        );
        assert_eq!(normalize_root_domain("localhost"), None);
    }

    #[test]
    fn normalizes_protocol_scheme() {
        assert_eq!(
            normalize_protocol_scheme("MAILTO:").as_deref(),
            Some("mailto")
        );
        assert_eq!(
            normalize_protocol_scheme("web+demo").as_deref(),
            Some("web+demo")
        );
        assert_eq!(normalize_protocol_scheme("bad scheme"), None);
    }
}
