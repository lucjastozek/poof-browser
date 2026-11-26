//! The sidebar UI: assembles its single HTML page from the files under `ui/` and creates its WebView.
//!
//! The order of `STYLES` and `SCRIPTS` matters: scripts share one global scope, and a script may
//! only use names declared by the ones before it.

use super::*;

const PAGE: &str = include_str!("../../ui/sidebar.html");

const STYLES: &[&str] = &[
    include_str!("../../ui/styles/foundation/theme.css"),
    include_str!("../../ui/styles/foundation/base.css"),
    include_str!("../../ui/styles/foundation/components.css"),
    include_str!("../../ui/styles/layout/topbar.css"),
    include_str!("../../ui/styles/layout/sidebar.css"),
    include_str!("../../ui/styles/layout/sidebar-pinned.css"),
    include_str!("../../ui/styles/layout/sidebar-folders.css"),
    include_str!("../../ui/styles/layout/sidebar-context-menu.css"),
    include_str!("../../ui/styles/layout/sidebar-split.css"),
    include_str!("../../ui/styles/layout/sidebar-password-offer.css"),
    include_str!("../../ui/styles/layout/stage.css"),
    include_str!("../../ui/styles/views/start-page.css"),
    include_str!("../../ui/styles/views/dialogs.css"),
    include_str!("../../ui/styles/views/settings.css"),
    include_str!("../../ui/styles/views/passwords.css"),
    include_str!("../../ui/styles/views/downloads.css"),
];

const SCRIPTS: &[&str] = &[
    include_str!("../../ui/icons.js"),
    include_str!("../../ui/scripts/core/dom.js"),
    include_str!("../../ui/scripts/core/space-icons.js"),
    include_str!("../../ui/scripts/core/widgets.js"),
    include_str!("../../ui/scripts/core/toast.js"),
    include_str!("../../ui/scripts/core/format.js"),
    include_str!("../../ui/scripts/core/colors.js"),
    include_str!("../../ui/scripts/core/theme.js"),
    include_str!("../../ui/scripts/core/state.js"),
    include_str!("../../ui/scripts/core/favicons.js"),
    include_str!("../../ui/scripts/core/views.js"),
    include_str!("../../ui/scripts/sidebar/chrome.js"),
    include_str!("../../ui/scripts/sidebar/folders.js"),
    include_str!("../../ui/scripts/sidebar/footer.js"),
    include_str!("../../ui/scripts/sidebar/context-menus.js"),
    include_str!("../../ui/scripts/sidebar/rows.js"),
    include_str!("../../ui/scripts/sidebar/state.js"),
    include_str!("../../ui/scripts/sidebar/address-bar.js"),
    include_str!("../../ui/scripts/sidebar/drag-reorder.js"),
    include_str!("../../ui/scripts/sidebar/split-drag.js"),
    include_str!("../../ui/scripts/sidebar/tab-switcher.js"),
    include_str!("../../ui/scripts/sidebar/edge-peek.js"),
    include_str!("../../ui/scripts/sidebar/shortcuts.js"),
    include_str!("../../ui/scripts/sidebar/password-offer.js"),
    include_str!("../../ui/scripts/sidebar/update-offer.js"),
    include_str!("../../ui/scripts/sidebar/ready.js"),
    include_str!("../../ui/scripts/views/start-page.js"),
    include_str!("../../ui/scripts/views/dialog.js"),
    include_str!("../../ui/scripts/views/svg-sanitizer.js"),
    include_str!("../../ui/scripts/views/space-editor.js"),
    include_str!("../../ui/scripts/views/extensions.js"),
    include_str!("../../ui/scripts/views/settings.js"),
    include_str!("../../ui/scripts/views/live-folder.js"),
    include_str!("../../ui/scripts/views/router.js"),
    include_str!("../../ui/scripts/views/history.js"),
    include_str!("../../ui/scripts/views/downloads.js"),
    include_str!("../../ui/scripts/views/default-browser.js"),
    include_str!("../../ui/scripts/views/passwords.js"),
];

/// The complete sidebar document, with all styles and scripts inlined.
pub(super) fn sidebar_html() -> String {
    let styles = format!("<style>{}</style>", STYLES.concat());
    let scripts: String = SCRIPTS
        .iter()
        .map(|script| format!("<script>{script}</script>"))
        .collect();
    PAGE.replace("/*STYLES*/", &styles)
        .replace("/*SCRIPTS*/", &scripts)
}

/// Creates the sidebar WebView. It is our own trusted UI, living in its own data folder.
pub(super) fn create_sidebar(
    window: &Window,
    proxy: &EventLoopProxy<UserEvent>,
    ctx: &mut WebContext,
) -> Result<WebView> {
    let ipc_proxy = proxy.clone();
    let builder = WebViewBuilder::new_with_web_context(ctx)
        .with_bounds(full_rect(window))
        .with_devtools(false)
        .with_html(sidebar_html())
        .with_ipc_handler(move |req| {
            let _ = ipc_proxy.send_event(UserEvent::Ui(req.into_body()));
        });
    #[cfg(windows)]
    let builder = {
        use wry::WebViewBuilderExtWindows;
        builder.with_additional_browser_args(WEBVIEW2_UI_ARGS)
    };
    let sidebar = builder.build_as_child(window)?;

    // The sidebar is a separate WebView, so it needs its own native
    // accelerator handler when it owns keyboard focus.
    #[cfg(windows)]
    register_browser_shortcuts(&sidebar, proxy.clone(), String::new());

    Ok(sidebar)
}
