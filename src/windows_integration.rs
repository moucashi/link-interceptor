use crate::models::{CandidateKind, OpenCandidate};
use std::{env, path::PathBuf, process::Command};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationState {
    NotRegistered,
    Registered,
    PossibleDefault,
}

#[derive(Debug, Clone)]
pub struct BrowserInfo {
    pub name: String,
    pub command: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProtocolInfo {
    pub name: String,
    pub command: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum IntegrationError {
    #[error("windows registry error: {0}")]
    Registry(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[cfg(not(windows))]
    #[error("operation is only available on Windows")]
    UnsupportedPlatform,
}

pub type Result<T> = std::result::Result<T, IntegrationError>;

#[cfg(windows)]
const APP_NAME: &str = "Link Interceptor";
#[cfg(windows)]
const REGISTERED_APP_NAME: &str = "LinkInterceptor";
#[cfg(windows)]
const CAPABILITIES_PATH: &str = r"Software\LinkInterceptor\Capabilities";
#[cfg(windows)]
const PROG_ID: &str = "LinkInterceptorURL";

#[cfg(windows)]
pub fn current_exe() -> Result<PathBuf> {
    Ok(env::current_exe()?)
}

#[cfg(not(windows))]
pub fn current_exe() -> Result<PathBuf> {
    Ok(env::current_exe()?)
}

#[cfg(windows)]
pub fn registration_state() -> RegistrationState {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let registered = hkcu
        .open_subkey(r"Software\RegisteredApplications")
        .and_then(|key| key.get_value::<String, _>(REGISTERED_APP_NAME))
        .map(|value| value == CAPABILITIES_PATH)
        .unwrap_or(false);

    if !registered {
        return RegistrationState::NotRegistered;
    }

    let http_default = read_user_choice("http").unwrap_or_default();
    let https_default = read_user_choice("https").unwrap_or_default();
    if http_default == PROG_ID || https_default == PROG_ID {
        RegistrationState::PossibleDefault
    } else {
        RegistrationState::Registered
    }
}

#[cfg(not(windows))]
pub fn registration_state() -> RegistrationState {
    RegistrationState::NotRegistered
}

#[cfg(windows)]
pub fn register_application() -> Result<()> {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};

    let exe = current_exe()?;
    let command = format!("\"{}\" \"%1\"", exe.display());
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    let (capabilities, _) = hkcu
        .create_subkey(CAPABILITIES_PATH)
        .map_err(registry_error)?;
    capabilities
        .set_value("ApplicationName", &APP_NAME)
        .map_err(registry_error)?;
    capabilities
        .set_value(
            "ApplicationDescription",
            &"Intercept URLs before opening them in a browser or app.",
        )
        .map_err(registry_error)?;
    let (url_associations, _) = capabilities
        .create_subkey("URLAssociations")
        .map_err(registry_error)?;
    url_associations
        .set_value("http", &PROG_ID)
        .map_err(registry_error)?;
    url_associations
        .set_value("https", &PROG_ID)
        .map_err(registry_error)?;

    let (registered_apps, _) = hkcu
        .create_subkey(r"Software\RegisteredApplications")
        .map_err(registry_error)?;
    registered_apps
        .set_value(REGISTERED_APP_NAME, &CAPABILITIES_PATH)
        .map_err(registry_error)?;

    let (prog_id, _) = hkcu
        .create_subkey(format!(r"Software\Classes\{PROG_ID}"))
        .map_err(registry_error)?;
    prog_id.set_value("", &APP_NAME).map_err(registry_error)?;
    prog_id
        .set_value("URL Protocol", &"")
        .map_err(registry_error)?;
    let (command_key, _) = prog_id
        .create_subkey(r"shell\open\command")
        .map_err(registry_error)?;
    command_key
        .set_value("", &command)
        .map_err(registry_error)?;

    Ok(())
}

#[cfg(not(windows))]
pub fn register_application() -> Result<()> {
    Err(IntegrationError::UnsupportedPlatform)
}

#[cfg(windows)]
pub fn unregister_application() -> Result<()> {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(registered_apps) = hkcu.open_subkey_with_flags(
        r"Software\RegisteredApplications",
        winreg::enums::KEY_WRITE | winreg::enums::KEY_READ,
    ) {
        let _ = registered_apps.delete_value(REGISTERED_APP_NAME);
    }
    let _ = hkcu.delete_subkey_all(r"Software\LinkInterceptor");
    let _ = hkcu.delete_subkey_all(format!(r"Software\Classes\{PROG_ID}"));
    Ok(())
}

#[cfg(not(windows))]
pub fn unregister_application() -> Result<()> {
    Err(IntegrationError::UnsupportedPlatform)
}

pub fn open_default_apps_settings() -> Result<()> {
    open::that("ms-settings:defaultapps").map_err(IntegrationError::Io)
}

#[cfg(windows)]
pub fn discover_browsers() -> Vec<BrowserInfo> {
    use winreg::{RegKey, enums::*};
    let mut browsers = Vec::new();
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let root = RegKey::predef(hive);
        if let Ok(start_menu) = root.open_subkey(r"Software\Clients\StartMenuInternet") {
            for key_name in start_menu.enum_keys().flatten() {
                if let Ok(key) = start_menu.open_subkey(&key_name) {
                    let name = key
                        .get_value::<String, _>("")
                        .ok()
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| key_name.clone());
                    let command = key
                        .open_subkey(r"shell\open\command")
                        .ok()
                        .and_then(|cmd| cmd.get_value::<String, _>("").ok());
                    push_unique_browser(&mut browsers, BrowserInfo { name, command });
                }
            }
        }
    }
    browsers
}

#[cfg(not(windows))]
pub fn discover_browsers() -> Vec<BrowserInfo> {
    Vec::new()
}

#[cfg(windows)]
pub fn discover_protocol_handler(scheme: &str) -> Option<ProtocolInfo> {
    use winreg::{RegKey, enums::*};
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let root = RegKey::predef(hive);
        let path = format!(r"Software\Classes\{scheme}");
        if let Ok(key) = root.open_subkey(path) {
            if key.get_value::<String, _>("URL Protocol").is_err() {
                continue;
            }
            let name = key
                .get_value::<String, _>("")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| format!("{scheme} handler"));
            let command = key
                .open_subkey(r"shell\open\command")
                .ok()
                .and_then(|cmd| cmd.get_value::<String, _>("").ok());
            return Some(ProtocolInfo { name, command });
        }
    }
    None
}

#[cfg(not(windows))]
pub fn discover_protocol_handler(_scheme: &str) -> Option<ProtocolInfo> {
    None
}

#[cfg(windows)]
pub fn discover_app_uri_handlers(domain: &str) -> Vec<OpenCandidate> {
    use winreg::{RegKey, enums::*};

    let mut candidates = Vec::new();
    let domain = domain.to_ascii_lowercase();
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let root = RegKey::predef(hive);
        if let Ok(system_app_data) = root.open_subkey(
            r"Software\Classes\LocalSettings\Software\Microsoft\Windows\CurrentVersion\AppModel\SystemAppData",
        ) {
            for app_key_name in system_app_data.enum_keys().flatten() {
                let Ok(app_key) = system_app_data.open_subkey(&app_key_name) else {
                    continue;
                };
                let Ok(handlers) = app_key.open_subkey("AppUriHandlers") else {
                    continue;
                };
                if app_uri_handlers_match_domain(&handlers, &domain) {
                    let mut candidate = OpenCandidate::new(
                        format!("App URI handler: {app_key_name}"),
                        CandidateKind::DomainApp,
                        None,
                        "{url}",
                        format!("Windows Apps for Websites match for {domain}"),
                    );
                    candidate.available = true;
                    candidates.push(candidate);
                }
            }
        }
    }
    candidates
}

#[cfg(not(windows))]
pub fn discover_app_uri_handlers(_domain: &str) -> Vec<OpenCandidate> {
    Vec::new()
}

#[cfg(windows)]
fn push_unique_browser(browsers: &mut Vec<BrowserInfo>, browser: BrowserInfo) {
    if browsers
        .iter()
        .any(|existing| existing.name.eq_ignore_ascii_case(&browser.name))
    {
        return;
    }
    browsers.push(browser);
}

#[cfg(windows)]
fn app_uri_handlers_match_domain(key: &winreg::RegKey, domain: &str) -> bool {
    for subkey in key.enum_keys().flatten() {
        if domain_pattern_matches(&subkey, domain) {
            return true;
        }
        if let Ok(child) = key.open_subkey(&subkey) {
            for value in child.enum_values().flatten() {
                if domain_pattern_matches(&value.0, domain) {
                    return true;
                }
                if let winreg::RegValue {
                    vtype: winreg::enums::RegType::REG_SZ,
                    bytes,
                } = value.1
                {
                    if let Ok(text) = String::from_utf16(
                        &bytes
                            .chunks_exact(2)
                            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                            .take_while(|unit| *unit != 0)
                            .collect::<Vec<_>>(),
                    ) {
                        if domain_pattern_matches(&text, domain) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    for value in key.enum_values().flatten() {
        if domain_pattern_matches(&value.0, domain) {
            return true;
        }
    }
    false
}

#[cfg(windows)]
fn domain_pattern_matches(pattern: &str, domain: &str) -> bool {
    let pattern = pattern.trim().to_ascii_lowercase();
    if pattern == domain {
        return true;
    }
    pattern
        .strip_prefix("*.")
        .is_some_and(|suffix| domain == suffix || domain.ends_with(&format!(".{suffix}")))
}

pub fn shell_fallback_candidate() -> OpenCandidate {
    OpenCandidate::new(
        "Windows default handler",
        CandidateKind::ShellFallback,
        None,
        "{url}",
        "Use the current Windows default handler",
    )
}

pub fn launch_candidate(candidate: &OpenCandidate, url: &str) -> Result<()> {
    if matches!(candidate.kind, CandidateKind::ShellFallback) || candidate.command.is_none() {
        open::that(url).map_err(IntegrationError::Io)?;
        return Ok(());
    }

    let command = candidate
        .command
        .as_ref()
        .expect("checked command presence");
    let mut parts = split_command_line(command);
    if parts.is_empty() {
        open::that(url).map_err(IntegrationError::Io)?;
        return Ok(());
    }
    let executable = parts.remove(0);
    let mut args = if candidate.args_template.trim().is_empty() {
        parts
    } else {
        split_command_line(&candidate.args_template)
    };
    if args.is_empty() {
        args.push("{url}".to_owned());
    }
    let args: Vec<String> = args
        .into_iter()
        .map(|arg| {
            arg.replace("%1", url)
                .replace("%L", url)
                .replace("{url}", url)
        })
        .collect();
    Command::new(executable).args(args).spawn()?;
    Ok(())
}

pub fn split_command_line(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => in_quotes = !in_quotes,
            '\\' if chars.peek() == Some(&'"') => {
                current.push('"');
                chars.next();
            }
            ch if ch.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

#[cfg(windows)]
fn read_user_choice(scheme: &str) -> Option<String> {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu.open_subkey(format!(
        r"Software\Microsoft\Windows\Shell\Associations\UrlAssociations\{scheme}\UserChoice"
    ))
    .ok()
    .and_then(|key| key.get_value("ProgId").ok())
}

#[cfg(windows)]
fn registry_error(error: std::io::Error) -> IntegrationError {
    IntegrationError::Registry(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_parser_handles_quoted_exe() {
        let parts = split_command_line(r#""C:\Program Files\App\app.exe" "%1" --flag"#);
        assert_eq!(parts[0], r"C:\Program Files\App\app.exe");
        assert_eq!(parts[1], "%1");
        assert_eq!(parts[2], "--flag");
    }
}
