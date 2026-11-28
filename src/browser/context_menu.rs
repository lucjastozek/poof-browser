//! The page's right-click menu. Windows only: WebView2 builds the menu and normally draws it with
//! the system's look. Here the request is held back (a deferral), its entries are listed, and the
//! page shows them in `js/context-menu.js`, styled like the sidebar's own menus. The choice comes
//! back as the id of one of WebView2's own commands, so Copy, Paste, Save as, Inspect and the rest
//! behave exactly as in the native menu. Entries made here (open in a new tab, copy a clean link,
//! search) use negative ids.
//!
//! The page is untrusted: its answer counts only with the one-time token made when the menu opens,
//! and only for the tab the menu belongs to.

use super::*;

/// Ids of the entries added here. WebView2's own command ids are positive.
pub(super) const OPEN_LINK: i32 = -1;
pub(super) const OPEN_IMAGE: i32 = -2;
pub(super) const COPY_LINK: i32 = -3;
pub(super) const SEARCH: i32 = -4;
/// A "Copy as" entry: the page sends the text to copy (selector, locator, cURL, ...).
pub(super) const COPY_AS: i32 = -5;
/// Longest text accepted for "Copy as".
const COPY_AS_MAX: usize = 4000;

/// What the pick asks native code to do beyond WebView2's own commands.
pub(super) struct Picked {
    pub action: i32,
    pub link: String,
    pub image: String,
    pub text: String,
}

/// Longest address accepted for the entries below (same bound as every tab URL).
const URL_MAX: usize = 4096;

#[cfg(windows)]
mod imp {
    use super::*;
    use std::cell::RefCell;
    use webview2_com::{ContextMenuRequestedEventHandler, Microsoft::Web::WebView2::Win32::*};
    use windows::core::{Interface, BOOL, PWSTR};
    use windows::Win32::Foundation::POINT;
    use windows::Win32::System::Com::CoTaskMemFree;

    /// The menu request that is waiting for an answer.
    struct Open {
        args: ICoreWebView2ContextMenuRequestedEventArgs,
        deferral: ICoreWebView2Deferral,
        tab: String,
        token: String,
        link: String,
        image: String,
        text: String,
    }

    thread_local! {
        static OPEN: RefCell<Option<Open>> = const { RefCell::new(None) };
    }

    /// Completes the held-back request. A positive id runs that WebView2 command; anything else
    /// just closes the menu (the request stays handled, so no native menu appears).
    fn complete(open: &Open, id: i32) {
        unsafe {
            if id > 0 {
                let _ = open.args.SetSelectedCommandId(id);
            }
            let _ = open.deferral.Complete();
        }
    }

    /// Gives the waiting request its token. Returns false when there is none for `tab`.
    pub(in crate::browser) fn arm(tab: &str, token: &str) -> bool {
        OPEN.with(|slot| match slot.borrow_mut().as_mut() {
            Some(open) if open.tab == tab && open.token.is_empty() => {
                open.token = token.to_string();
                true
            }
            _ => false,
        })
    }

    /// The page chose (`id` 0 = dismissed). Needs the token the menu was armed with.
    pub(in crate::browser) fn finish(tab: &str, token: &str, id: i32) -> Option<Picked> {
        let open = OPEN.with(|slot| {
            let mut slot = slot.borrow_mut();
            match slot.as_ref() {
                Some(o) if o.tab == tab && !o.token.is_empty() && o.token == token => slot.take(),
                _ => None,
            }
        })?;
        complete(&open, id);
        Some(Picked {
            action: if id < 0 { id } else { 0 },
            link: open.link,
            image: open.image,
            text: open.text,
        })
    }

    /// Drops a menu that can no longer be shown (the tab went away, the script could not run).
    pub(in crate::browser) fn cancel() {
        if let Some(open) = OPEN.with(|slot| slot.borrow_mut().take()) {
            complete(&open, 0);
        }
    }

    pub(in crate::browser) fn watch_context_menu(
        wv: &WebView,
        proxy: &EventLoopProxy<UserEvent>,
        tab: &str,
    ) {
        let proxy = proxy.clone();
        let tab = tab.to_string();
        unsafe {
            let Ok(core) = wv.controller().CoreWebView2() else {
                return;
            };
            let Ok(core11) = core.cast::<ICoreWebView2_11>() else {
                log::warn!("WebView2 runtime lacks context menu support (ICoreWebView2_11)");
                return;
            };
            let handler = ContextMenuRequestedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else { return Ok(()) };
                // If anything fails before the request is taken over, WebView2 shows its own menu.
                if let Err(error) = take_over(&args, &proxy, &tab) {
                    log::warn!("custom context menu unavailable, using the native one: {error}");
                }
                Ok(())
            }));
            let mut token = 0;
            if let Err(error) = core11.add_ContextMenuRequested(&handler, &mut token) {
                log::warn!("registering WebView2 context menu handler failed: {error}");
            }
        }
    }

    unsafe fn read(get: impl FnOnce(*mut PWSTR) -> windows::core::Result<()>) -> String {
        let mut value = PWSTR::null();
        if get(&mut value as *mut PWSTR).is_err() || value.is_null() {
            return String::new();
        }
        let text = value.to_string().unwrap_or_default();
        CoTaskMemFree(Some(value.0 as *const std::ffi::c_void));
        text
    }

    unsafe fn flag(get: impl FnOnce(*mut BOOL) -> windows::core::Result<()>) -> bool {
        let mut value = BOOL(0);
        get(&mut value as *mut BOOL).is_ok() && value.as_bool()
    }

    /// WebView2's entries as plain data for the page. `k`: s separator, i item, c check box,
    /// r radio, x submenu.
    unsafe fn describe(items: &ICoreWebView2ContextMenuItemCollection, depth: u8) -> Vec<Value> {
        let mut count = 0u32;
        let _ = items.Count(&mut count);
        let mut out = Vec::new();
        for index in 0..count {
            let Ok(item) = items.GetValueAtIndex(index) else {
                continue;
            };
            let mut kind = COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_COMMAND;
            let _ = item.Kind(&mut kind);
            if kind == COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR {
                out.push(json!({ "k": "s" }));
                continue;
            }
            let label = read(|p| item.Label(p));
            let name = read(|p| item.Name(p)).to_lowercase();
            // Replaced by entries made here, or already covered by the browser (screenshot).
            if label.is_empty()
                || name.contains("newwindow")
                || name.contains("webcapture")
                || name.contains("share")
            {
                continue;
            }
            let mut id = 0i32;
            let _ = item.CommandId(&mut id);
            let mut enabled_flag = BOOL(1);
            let _ = item.IsEnabled(&mut enabled_flag);
            let enabled = enabled_flag.as_bool();
            let checked = flag(|p| item.IsChecked(p));
            let hint = read(|p| item.ShortcutKeyDescription(p));
            if kind == COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SUBMENU {
                let children = if depth < 1 {
                    item.Children()
                        .map(|c| describe(&c, depth + 1))
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                out.push(json!({ "k": "x", "l": label, "c": children, "e": enabled }));
                continue;
            }
            let k = if kind == COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_CHECK_BOX {
                "c"
            } else if kind == COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_RADIO {
                "r"
            } else {
                "i"
            };
            out.push(json!({ "k": k, "i": id, "l": label, "h": hint, "e": enabled, "v": checked }));
        }
        out
    }

    unsafe fn take_over(
        args: &ICoreWebView2ContextMenuRequestedEventArgs,
        proxy: &EventLoopProxy<UserEvent>,
        tab: &str,
    ) -> windows::core::Result<()> {
        let target = args.ContextMenuTarget()?;
        let mut kind = COREWEBVIEW2_CONTEXT_MENU_TARGET_KIND_PAGE;
        let _ = target.Kind(&mut kind);
        let link = read(|p| target.LinkUri(p));
        let link = if ok_url(&link, URL_MAX, false) {
            link
        } else {
            String::new()
        };
        let image = if kind == COREWEBVIEW2_CONTEXT_MENU_TARGET_KIND_IMAGE {
            read(|p| target.SourceUri(p))
        } else {
            String::new()
        };
        let image = if ok_url(&image, URL_MAX, false) {
            image
        } else {
            String::new()
        };
        let editable = flag(|p| target.IsEditable(p));
        let text = if flag(|p| target.HasSelection(p)) && !editable {
            read(|p| target.SelectionText(p))
                .chars()
                .take(300)
                .collect::<String>()
        } else {
            String::new()
        };

        let mut entries: Vec<Value> = Vec::new();
        if !link.is_empty() {
            entries
                .push(json!({ "k": "i", "i": OPEN_LINK, "l": "Open link in new tab", "e": true }));
            entries.push(json!({ "k": "i", "i": COPY_LINK, "l": "Copy clean link", "e": true }));
        }
        if !image.is_empty() {
            entries.push(
                json!({ "k": "i", "i": OPEN_IMAGE, "l": "Open image in new tab", "e": true }),
            );
        }
        let search = text.trim();
        if !search.is_empty() {
            let short: String = search.chars().take(24).collect();
            let dots = if search.chars().count() > 24 {
                "…"
            } else {
                ""
            };
            entries.push(json!({ "k": "i", "i": SEARCH, "l": format!("Search for “{short}{dots}”"), "e": true }));
        }
        if !entries.is_empty() {
            entries.push(json!({ "k": "s" }));
        }
        entries.extend(describe(&args.MenuItems()?, 0));

        let mut at = POINT::default();
        let _ = args.Location(&mut at);
        let deferral = args.GetDeferral()?;
        let target = if !link.is_empty() { &link } else { &image };
        let body = json!({
            "t": "ctx_open", "tab": tab, "items": entries, "x": at.x, "y": at.y, "u": target,
        })
        .to_string();
        let open = Open {
            args: args.clone(),
            deferral,
            tab: tab.to_string(),
            token: String::new(),
            link,
            image,
            text,
        };
        // A menu still waiting (it was never shown, or the page ignored it) is closed first.
        if let Some(old) = OPEN.with(|slot| slot.borrow_mut().replace(open)) {
            complete(&old, 0);
        }
        // From here on the request is ours: no native menu unless the page's menu fails to show.
        args.SetHandled(true)?;
        if proxy
            .send_event(UserEvent::Page {
                tab: String::new(),
                body,
            })
            .is_err()
        {
            cancel();
        }
        Ok(())
    }
}

#[cfg(windows)]
pub(super) use imp::{arm, cancel, finish, watch_context_menu};

#[cfg(not(windows))]
pub(super) fn watch_context_menu(_: &WebView, _: &EventLoopProxy<UserEvent>, _: &str) {}
#[cfg(not(windows))]
pub(super) fn arm(_: &str, _: &str) -> bool {
    false
}
#[cfg(not(windows))]
pub(super) fn cancel() {}
#[cfg(not(windows))]
pub(super) fn finish(_: &str, _: &str, _: i32) -> Option<Picked> {
    None
}

impl Browser {
    /// Native code listed a menu for `tab`: show it in that page (internal message, empty tab id).
    pub(super) fn on_context_menu_open(
        &mut self,
        tab: &str,
        items: &Value,
        x: f64,
        y: f64,
        target: &str,
    ) {
        let token = uuid::Uuid::new_v4().simple().to_string();
        let Some(wv) = self
            .tabs
            .iter()
            .find(|t| t.id == tab)
            .and_then(|t| t.webview.as_ref())
        else {
            context_menu::cancel();
            return;
        };
        if !context_menu::arm(tab, &token) {
            return;
        }
        let accent = self.cur_space().color.clone();
        let accent = if ok_color(&accent) {
            accent
        } else {
            "#6dc5e2".into()
        };
        let theme = match self.p.theme.as_str() {
            "light" => "light",
            "dark" => "dark",
            _ => "system",
        };
        let target = if ok_url(target, URL_MAX, false) {
            target
        } else {
            ""
        };
        let script = CONTEXT_MENU_JS
            .replace("__LOCATORS__", LOCATORS_JS.trim_end().trim_end_matches(';'))
            .replace(
                "__TARGET__",
                &json!(target).to_string().replace('<', "\\u003c"),
            )
            .replace("__NONCE__", &token)
            .replace("__ACCENT__", &accent)
            .replace("__THEME__", theme)
            .replace("__X__", &format!("{}", x.round() as i64))
            .replace("__Y__", &format!("{}", y.round() as i64))
            // Last, so nothing inside the list can be mistaken for a placeholder. `<` is escaped so
            // a label can never close the script.
            .replace("__ITEMS__", &items.to_string().replace('<', "\\u003c"));
        if wv.evaluate_script(&script).is_err() {
            context_menu::cancel();
        }
    }

    /// The page answered. UNTRUSTED: `finish` checks tab and token.
    pub(super) fn on_context_menu_pick(
        &mut self,
        tab: &str,
        token: &str,
        id: i32,
        copy: &str,
        label: &str,
    ) {
        let Some(pick) = context_menu::finish(tab, token, id) else {
            return;
        };
        match pick.action {
            context_menu::OPEN_LINK if ok_url(&pick.link, URL_MAX, false) => {
                if let Err(e) = self.open_background_tab(&pick.link) {
                    log::error!("{e:#}");
                }
            }
            context_menu::OPEN_IMAGE if ok_url(&pick.image, URL_MAX, false) => {
                if let Err(e) = self.open_background_tab(&pick.image) {
                    log::error!("{e:#}");
                }
            }
            context_menu::COPY_LINK if ok_url(&pick.link, URL_MAX, false) => {
                let (link, removed) = clean_link::clean_link(&pick.link);
                let message = match removed {
                    0 => "Link copied".to_string(),
                    1 => "Clean link copied, 1 tracker removed".to_string(),
                    n => format!("Clean link copied, {n} trackers removed"),
                };
                let _ = self.sidebar.focus();
                crate::gesture::eval_with_gesture(
                    &self.sidebar,
                    &format!(
                        "window.__copyLink&&window.__copyLink({},{},{})",
                        json!(link),
                        json!(message),
                        json!(tab)
                    ),
                );
            }
            context_menu::COPY_AS if !copy.is_empty() => {
                let text: String = copy
                    .chars()
                    .filter(|c| !c.is_control() || *c == '\n')
                    .take(COPY_AS_MAX)
                    .collect();
                let label: String = label.chars().filter(|c| !c.is_control()).take(40).collect();
                let _ = self.sidebar.focus();
                crate::gesture::eval_with_gesture(
                    &self.sidebar,
                    &format!(
                        "window.__copyLink&&window.__copyLink({},{},{})",
                        json!(text),
                        json!(format!("{label} copied")),
                        json!(tab)
                    ),
                );
            }
            context_menu::SEARCH if !pick.text.trim().is_empty() => {
                let url = normalize(pick.text.trim(), &self.p.search_engine);
                if let Err(e) = self.new_tab(&url) {
                    log::error!("{e:#}");
                }
            }
            _ => {}
        }
    }
}
