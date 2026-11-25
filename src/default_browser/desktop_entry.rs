//! The freedesktop `.desktop` file that lets Linux list poof as a web browser. Pure text.

use super::APP_NAME;
use std::path::Path;

/// File name of the entry, as `xdg-settings` wants it.
pub const FILE_NAME: &str = "poof.desktop";

/// An `Exec` argument: quoted when needed, with the characters the spec reserves escaped.
fn exec_arg(arg: &str) -> String {
    if !arg.chars().any(|c| " \t\n\"'\\><~|&;$*?#()`".contains(c)) {
        return arg.to_string();
    }
    let escaped: String = arg
        .chars()
        .flat_map(|c| match c {
            '"' | '`' | '$' | '\\' => vec!['\\', c],
            _ => vec![c],
        })
        .collect();
    format!("\"{escaped}\"")
}

pub fn desktop_entry(exe: &Path) -> String {
    let exe = exec_arg(&exe.display().to_string());
    format!(
        "[Desktop Entry]\nType=Application\nName={APP_NAME}\nGenericName=Web Browser\n\
         Comment=A calm, fast web browser\nExec={exe} %U\nTerminal=false\n\
         Categories=Network;WebBrowser;\n\
         MimeType=text/html;application/xhtml+xml;x-scheme-handler/http;x-scheme-handler/https;\n\
         StartupNotify=true\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_lists_the_browser_mime_types() {
        let text = desktop_entry(Path::new("/opt/poof/poof"));
        assert!(text.contains("Exec=/opt/poof/poof %U\n"));
        assert!(text.contains("x-scheme-handler/https"));
        assert!(text.contains("Categories=Network;WebBrowser;"));
    }

    #[test]
    fn paths_with_spaces_are_quoted() {
        let text = desktop_entry(Path::new("/opt/my poof/poof"));
        assert!(text.contains("Exec=\"/opt/my poof/poof\" %U\n"));
    }
}
