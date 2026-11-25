//! The Windows registration, as text: a `.reg` file for `reg import`, and parsers for the
//! output of `reg query`. Pure, so it is tested on every platform.

use super::APP_NAME;
use std::path::Path;

/// Identifier of the document/URL handler class (the "ProgId") poof registers.
pub const PROG_ID: &str = "PoofHTML";
/// Key under `HKCU\Software\Clients\StartMenuInternet` that lists poof as a browser.
pub const CLIENT_KEY: &str = "Poof";

/// File types poof offers to open.
const FILE_TYPES: &[&str] = &[".htm", ".html", ".xhtml", ".pdf"];
/// Link schemes poof offers to open.
const URL_SCHEMES: &[&str] = &["http", "https"];

/// A value in a `.reg` file: quotes and backslashes escaped.
fn quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The command line Windows runs to open a link or file: `"poof.exe" "%1"`.
pub fn open_command(exe: &Path) -> String {
    format!("\"{}\" \"%1\"", exe.display())
}

/// The complete registration. Everything lives under HKEY_CURRENT_USER, so no administrator
/// rights are needed.
pub fn reg_file(exe: &Path) -> String {
    let exe_text = exe.display().to_string();
    let icon = format!("{exe_text},0");
    let capabilities =
        format!(r"HKEY_CURRENT_USER\Software\Clients\StartMenuInternet\{CLIENT_KEY}\Capabilities");
    let class = format!(r"HKEY_CURRENT_USER\Software\Classes\{PROG_ID}");
    let client = format!(r"HKEY_CURRENT_USER\Software\Clients\StartMenuInternet\{CLIENT_KEY}");

    let mut out = String::from("Windows Registry Editor Version 5.00\r\n\r\n");
    let mut section = |path: &str, values: &[(&str, String)]| {
        out.push_str(&format!("[{path}]\r\n"));
        for (name, value) in values {
            let name = if name.is_empty() {
                "@".to_string()
            } else {
                quoted(name)
            };
            out.push_str(&format!("{name}={}\r\n", quoted(value)));
        }
        out.push_str("\r\n");
    };

    section(
        &class,
        &[
            ("", format!("{APP_NAME} document")),
            ("FriendlyTypeName", format!("{APP_NAME} document")),
        ],
    );
    section(
        &format!(r"{class}\Application"),
        &[
            ("ApplicationName", APP_NAME.into()),
            ("ApplicationIcon", icon.clone()),
            ("ApplicationDescription", "A calm, fast web browser".into()),
        ],
    );
    section(&format!(r"{class}\DefaultIcon"), &[("", icon.clone())]);
    section(
        &format!(r"{class}\shell\open\command"),
        &[("", open_command(exe))],
    );

    section(&client, &[("", APP_NAME.into())]);
    section(
        &capabilities,
        &[
            ("ApplicationName", APP_NAME.into()),
            ("ApplicationDescription", "A calm, fast web browser".into()),
            ("ApplicationIcon", icon.clone()),
        ],
    );
    fn associations<'a>(types: &[&'a str]) -> Vec<(&'a str, String)> {
        types.iter().map(|t| (*t, PROG_ID.to_string())).collect()
    }
    section(
        &format!(r"{capabilities}\FileAssociations"),
        &associations(FILE_TYPES),
    );
    section(
        &format!(r"{capabilities}\URLAssociations"),
        &associations(URL_SCHEMES),
    );
    section(
        &format!(r"{capabilities}\StartMenu"),
        &[("StartMenuInternet", CLIENT_KEY.into())],
    );
    section(&format!(r"{client}\DefaultIcon"), &[("", icon)]);
    section(
        &format!(r"{client}\shell\open\command"),
        &[("", format!("\"{exe_text}\""))],
    );
    section(
        r"HKEY_CURRENT_USER\Software\RegisteredApplications",
        &[(
            APP_NAME,
            format!(r"Software\Clients\StartMenuInternet\{CLIENT_KEY}\Capabilities"),
        )],
    );
    out
}

/// A `.reg` file must be UTF-16 with a byte-order mark to carry non-ASCII paths.
pub fn utf16_le_with_bom(text: &str) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}

/// The data of the named value in `reg query` output (`name  REG_SZ  data`).
pub fn query_value<'a>(output: &'a str, name: &str) -> Option<&'a str> {
    output.lines().find_map(|line| {
        let line = line.trim();
        let rest = line.strip_prefix(name)?.trim_start();
        let data = rest.strip_prefix("REG_SZ")?;
        Some(data.trim())
    })
}

/// The command a `reg query ... /ve` output holds for the default value.
pub fn query_default_value(output: &str) -> Option<&str> {
    query_value(output, "(Default)").filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_covers_links_and_files() {
        let text = reg_file(Path::new(r"C:\Apps\poof\poof.exe"));
        assert!(text.starts_with("Windows Registry Editor Version 5.00\r\n"));
        assert!(text.contains(r#""https"="PoofHTML""#));
        assert!(text.contains(r#"".html"="PoofHTML""#));
        assert!(text.contains(r#"@="\"C:\\Apps\\poof\\poof.exe\" \"%1\"""#));
        assert!(text.contains(r"[HKEY_CURRENT_USER\Software\RegisteredApplications]"));
        assert!(
            text.contains(r#""poof"="Software\\Clients\\StartMenuInternet\\Poof\\Capabilities""#)
        );
    }

    #[test]
    fn values_are_escaped() {
        assert_eq!(quoted(r#"a\b"c"#), r#""a\\b\"c""#);
    }

    #[test]
    fn utf16_has_a_bom_and_keeps_unicode() {
        let bytes = utf16_le_with_bom("aé");
        assert_eq!(&bytes[..2], &[0xFF, 0xFE]);
        assert_eq!(&bytes[2..], &[b'a', 0, 0xE9, 0]);
    }

    #[test]
    fn query_output_is_parsed() {
        let out = "\r\nHKEY_CURRENT_USER\\x\\UserChoice\r\n    ProgId    REG_SZ    PoofHTML\r\n    Hash    REG_SZ    abc=\r\n";
        assert_eq!(query_value(out, "ProgId"), Some("PoofHTML"));
        assert_eq!(query_value(out, "Missing"), None);
        let cmd = "HKEY_CURRENT_USER\\k\r\n    (Default)    REG_SZ    \"C:\\Program Files\\poof.exe\"\r\n";
        assert_eq!(
            query_default_value(cmd),
            Some("\"C:\\Program Files\\poof.exe\"")
        );
        assert_eq!(query_default_value("    (Default)    REG_SZ    \r\n"), None);
    }
}
