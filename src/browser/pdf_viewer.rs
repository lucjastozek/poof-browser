//! The built-in PDF viewer and PDF tools.
//!
//! The viewer is a web app (`ui/pdf/`) served from memory by the `poofpdf` custom protocol to
//! any tab WebView. Navigations to a `.pdf` address (web or local file) are cancelled and the tab
//! loads the viewer instead. The viewer cannot fetch other sites itself (no CORS), so it asks
//! `/fetch`, which downloads the file here and hands the bytes back.

use super::*;
use std::{borrow::Cow, io::Read};
use wry::http::{header, Request, Response, StatusCode};

const SCHEME: &str = "poofpdf";
/// Windows serves custom protocols as `http://<scheme>.localhost`, the other platforms as `<scheme>://localhost`.
#[cfg(windows)]
const ORIGIN: &str = "http://poofpdf.localhost";
#[cfg(not(windows))]
const ORIGIN: &str = "poofpdf://localhost";

/// Largest PDF the viewer loads, so a bad link cannot fill memory.
const PDF_MAX_BYTES: u64 = 512 * 1024 * 1024;
const FETCH_TIMEOUT: Duration = Duration::from_secs(60);
/// Header every viewer fetch carries. A web page cannot send it without a CORS preflight, which
/// this protocol never approves, so only the viewer can use `/fetch`.
const FETCH_HEADER: &str = "x-poof-pdf";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) poof";

struct Asset {
    path: &'static str,
    mime: &'static str,
    body: &'static [u8],
}

const HTML: &str = "text/html; charset=utf-8";
const JS: &str = "text/javascript; charset=utf-8";
const CSS: &str = "text/css; charset=utf-8";

macro_rules! asset {
    ($path:literal, $mime:expr, $file:literal) => {
        Asset {
            path: $path,
            mime: $mime,
            body: include_bytes!(concat!("../../ui/", $file)),
        }
    };
}

/// Everything the viewer page loads. New viewer files must be listed here and in `ui/pdf/index.html`.
const ASSETS: &[Asset] = &[
    asset!("/index.html", HTML, "pdf/index.html"),
    asset!("/theme.css", CSS, "styles/foundation/theme.css"),
    asset!("/pdf.css", CSS, "pdf/pdf.css"),
    asset!("/tools.css", CSS, "pdf/tools.css"),
    asset!("/vendor/pdf.min.js", JS, "pdf/vendor/pdf.min.js"),
    asset!(
        "/vendor/pdf.worker.min.js",
        JS,
        "pdf/vendor/pdf.worker.min.js"
    ),
    asset!("/vendor/pdf-lib.min.js", JS, "pdf/vendor/pdf-lib.min.js"),
    asset!("/js/core.js", JS, "pdf/js/core.js"),
    asset!("/js/zip.js", JS, "pdf/js/zip.js"),
    asset!("/js/geom.js", JS, "pdf/js/geom.js"),
    asset!("/js/viewer.js", JS, "pdf/js/viewer.js"),
    asset!("/js/viewer-panels.js", JS, "pdf/js/viewer-panels.js"),
    asset!("/js/viewer-search.js", JS, "pdf/js/viewer-search.js"),
    asset!("/js/annotate.js", JS, "pdf/js/annotate.js"),
    asset!("/js/annotate-export.js", JS, "pdf/js/annotate-export.js"),
    asset!("/js/sign.js", JS, "pdf/js/sign.js"),
    asset!("/js/tools-ui.js", JS, "pdf/js/tools-ui.js"),
    asset!("/js/pagegrid.js", JS, "pdf/js/pagegrid.js"),
    asset!("/js/tools-pages.js", JS, "pdf/js/tools-pages.js"),
    asset!("/js/tools-convert.js", JS, "pdf/js/tools-convert.js"),
    asset!("/js/tools-edit.js", JS, "pdf/js/tools-edit.js"),
    asset!("/js/tools-optimize.js", JS, "pdf/js/tools-optimize.js"),
    asset!("/js/app.js", JS, "pdf/js/app.js"),
];

/// The viewer's start page: drop zone and tool grid.
pub(super) fn home_url() -> String {
    format!("{ORIGIN}/")
}

/// The viewer showing `source`, a web or `file:` address of a PDF.
fn viewer_url(source: &str) -> String {
    let query: String = url::form_urlencoded::byte_serialize(source.as_bytes()).collect();
    format!("{ORIGIN}/?u={query}")
}

/// The viewer address for a link that points at a PDF file, or `None` for any other address.
pub(super) fn viewer_for(address: &str) -> Option<String> {
    let parsed = url::Url::parse(address).ok()?;
    let is_pdf = parsed.path().to_ascii_lowercase().ends_with(".pdf");
    let known_scheme = matches!(parsed.scheme(), "http" | "https" | "file");
    (is_pdf && known_scheme).then(|| viewer_url(address))
}

/// Navigation hook of a tab: sends `.pdf` addresses to the viewer. Returns whether the
/// navigation may go on.
pub(super) fn allow_navigation(
    address: &str,
    tab: &str,
    proxy: &EventLoopProxy<UserEvent>,
) -> bool {
    let Some(viewer) = viewer_for(address) else {
        return true;
    };
    let _ = proxy.send_event(UserEvent::OpenPdf {
        tab: tab.to_string(),
        url: viewer,
    });
    false
}

/// Adds the `poofpdf` protocol to a tab's WebView.
pub(super) fn register(builder: WebViewBuilder<'_>) -> WebViewBuilder<'_> {
    builder.with_asynchronous_custom_protocol(SCHEME.into(), |_id, request, responder| {
        if request.uri().path() == "/fetch" {
            fetch(request, responder);
        } else {
            responder.respond(serve_asset(request.uri().path()));
        }
    })
}

fn serve_asset(path: &str) -> Response<Cow<'static, [u8]>> {
    let path = if path == "/" { "/index.html" } else { path };
    match ASSETS.iter().find(|asset| asset.path == path) {
        Some(asset) => reply(StatusCode::OK, asset.mime, Cow::Borrowed(asset.body)),
        None => reply(
            StatusCode::NOT_FOUND,
            "text/plain",
            Cow::Borrowed(b"not found"),
        ),
    }
}

/// Downloads (or reads) the PDF named by `?u=` on a short thread, so the UI never waits for it.
fn fetch(request: Request<Vec<u8>>, responder: wry::RequestAsyncResponder) {
    let target = request.uri().query().and_then(|query| {
        url::form_urlencoded::parse(query.as_bytes())
            .find(|(key, _)| key == "u")
            .map(|(_, value)| value.into_owned())
    });
    let (Some(target), true) = (target, request.headers().contains_key(FETCH_HEADER)) else {
        let denied = reply(
            StatusCode::FORBIDDEN,
            "text/plain",
            Cow::Borrowed(b"denied"),
        );
        return responder.respond(denied);
    };
    std::thread::spawn(move || {
        responder.respond(match read_pdf(&target) {
            Ok(bytes) => reply(StatusCode::OK, "application/pdf", Cow::Owned(bytes)),
            Err(error) => {
                log::warn!("opening a PDF failed: {error:#}");
                let message = Cow::Owned(error.to_string().into_bytes());
                reply(StatusCode::BAD_GATEWAY, "text/plain", message)
            }
        });
    });
}

fn read_pdf(target: &str) -> Result<Vec<u8>> {
    let address = url::Url::parse(target)?;
    let mut bytes = Vec::new();
    match address.scheme() {
        "file" => {
            let path = address
                .to_file_path()
                .map_err(|()| anyhow::anyhow!("not a local file address"))?;
            let is_pdf = path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"));
            anyhow::ensure!(is_pdf, "only PDF files can be opened here");
            std::fs::File::open(path)?
                .take(PDF_MAX_BYTES + 1)
                .read_to_end(&mut bytes)?;
        }
        "http" | "https" => {
            let agent = ureq::AgentBuilder::new()
                .timeout(FETCH_TIMEOUT)
                .user_agent(USER_AGENT)
                .build();
            agent
                .get(address.as_str())
                .call()?
                .into_reader()
                .take(PDF_MAX_BYTES + 1)
                .read_to_end(&mut bytes)?;
        }
        other => anyhow::bail!("unsupported address type: {other}"),
    }
    anyhow::ensure!(bytes.len() as u64 <= PDF_MAX_BYTES, "the file is too large");
    Ok(bytes)
}

fn reply(status: StatusCode, mime: &str, body: Cow<'static, [u8]>) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime)
        .body(body)
        .unwrap_or_else(|_| Response::new(Cow::Borrowed(&[])))
}

impl Browser {
    /// Loads the viewer into the tab that tried to open a PDF.
    pub(super) fn open_pdf_in_tab(&mut self, tab: &str, viewer: &str) {
        let Some(wv) = self
            .tabs
            .iter()
            .find(|t| t.id == tab)
            .and_then(|t| t.webview.as_ref())
        else {
            return;
        };
        if let Err(error) = wv.load_url(viewer) {
            log::warn!("loading the PDF viewer failed: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_pdf_files_go_to_the_viewer() {
        assert!(viewer_for("https://example.com/a/Report.PDF").is_some());
        assert!(viewer_for("file:///C:/docs/a.pdf").is_some());
        assert!(viewer_for("https://example.com/a.pdf.html").is_none());
        assert!(viewer_for("https://example.com/viewer?file=a.pdf").is_none());
        assert!(viewer_for("javascript:alert(1)//.pdf").is_none());
    }
}
