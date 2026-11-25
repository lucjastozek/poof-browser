//! Commands from the sidebar UI. Short ones are handled inline; longer ones live next to the
//! state they change (spaces, folders, history, downloads, profile cleanup, DevTools).

use super::*;

impl Browser {
    /// Executes one command from the (trusted) sidebar UI.
    pub(super) fn run(&mut self, cmd: Cmd) -> Result<()> {
        match cmd {
            Cmd::Ready => {
                // The sidebar just (re)loaded and has no state yet: force a full push.
                self.last_state_hash.set(0);
                self.tab_drag = false;
                self.sidebar_ready = true;
                self.restore_space_tab()?;
                self.push_downloads();
                self.push_update_offer();
                self.open_startup_urls();
            }
            Cmd::SidebarCollapsed { collapsed } => {
                self.p.sidebar_collapsed = collapsed;
                self.peek = false;
                self.save();
                self.relayout();
            }
            Cmd::SidebarCompact { compact } => {
                self.p.sidebar_compact = compact;
                if compact {
                    self.p.sidebar_collapsed = false;
                }
                self.save();
                self.relayout();
            }
            Cmd::SidebarWidth { width, persist } => {
                if width.is_finite() {
                    self.p.sidebar_width = width.clamp(SIDEBAR_MIN_W, SIDEBAR_MAX_W);
                    if persist {
                        self.save();
                    }
                    self.relayout();
                }
            }
            Cmd::SidebarPeek { open } => {
                let want = open && self.p.sidebar_collapsed && !self.fullscreen;
                if want != self.peek {
                    self.peek = want;
                    self.relayout();
                }
            }
            Cmd::SplitTab {
                id,
                target,
                stacked,
                first,
            } => self.open_split(&id, target.as_deref(), stacked, first)?,
            Cmd::TabDrag { active } => {
                if self.tab_drag != active {
                    self.tab_drag = active;
                    self.relayout();
                }
            }
            Cmd::SplitClose { id } => match id {
                Some(id) if !self.split_has(&id) => {
                    self.saved_splits.retain(|s| s.a != id && s.b != id)
                }
                _ => self.close_split(),
            },
            Cmd::SplitOrientation => self.toggle_split_orientation(),
            Cmd::SplitRatio { ratio } => {
                if ratio.is_finite() {
                    if let Some(sp) = self.split.as_mut() {
                        sp.ratio = ratio.clamp(0.15, 0.85);
                    }
                    self.relayout();
                }
            }
            Cmd::ToggleScrollbar => self.toggle_scrollbar(),
            Cmd::FullscreenToggle => self.toggle_page_fullscreen(),
            Cmd::FullscreenLeave => self.leave_fullscreen(),
            Cmd::PasswordOfferAccept => self.accept_password_offer(),
            Cmd::PasswordOfferDismiss { never } => self.dismiss_password_offer(never),
            Cmd::ListDownloads => self.list_downloads(),
            Cmd::PauseDownload { id } => self.pause_download(&id),
            Cmd::ResumeDownload { id } => self.resume_download(&id),
            Cmd::CancelDownload { id } => self.cancel_download(&id),
            Cmd::OpenDownload { id } => self.open_download(&id),
            Cmd::ShowDownload { id } => self.show_download(&id),
            Cmd::RemoveDownload { id } => self.remove_download(&id),
            Cmd::ClearDownloads => self.clear_downloads(),
            Cmd::DefaultBrowserStatus => self.push_default_browser_status(),
            Cmd::SetDefaultBrowser => self.set_default_browser(),
            Cmd::ListPasswords => self.list_passwords(),
            Cmd::RevealPassword { id } => self.reveal_password(&id),
            Cmd::DeletePassword { id } => self.delete_password(&id),
            Cmd::SetSavePasswords { on } => {
                self.p.save_passwords = on;
                self.save();
            }
            Cmd::WindowMinimize => self.window.set_minimized(true),
            Cmd::WindowToggleMaximize => self.window.set_maximized(!self.window.is_maximized()),
            Cmd::WindowDrag => {
                log::debug!("window_drag received");
                if let Err(e) = self.window.drag_window() {
                    log::warn!("drag_window failed: {e}");
                }
            }
            Cmd::WindowClose => {
                let _ = self.proxy.send_event(UserEvent::CloseWindow);
            }
            Cmd::Overlay { open } => {
                self.overlay = open;
                if open {
                    let _ = self.sidebar.focus();
                }
                self.relayout();
                if !open {
                    if let Some(wv) = self.visible_webview() {
                        let _ = wv.focus();
                    }
                }
            }
            Cmd::NewTab { input } => self.new_tab(&normalize(&input))?,
            Cmd::Navigate { input } => {
                let url = normalize(&input);
                if let Some(wv) = self.visible_webview() {
                    wv.load_url(&url)?;
                } else {
                    self.new_tab(&url)?;
                }
            }
            Cmd::SelectTab { id } => self.activate(&id)?,
            Cmd::CloseTab { id } => self.close_tab(&id)?,
            Cmd::UnloadTab { id } => self.unload_tab(&id)?,
            Cmd::ClearTabs => self.clear_tabs()?,
            Cmd::PinTab { id } => self.set_kind(&id, Kind::Pinned),
            Cmd::UnpinTab { id } | Cmd::UnfavTab { id } => self.set_kind(&id, Kind::Normal),
            Cmd::FavTab { id } => self.set_kind(&id, Kind::Favorite),
            Cmd::SwitchSpace { id } => self.switch_space(&id)?,
            Cmd::CycleSpace { dir } => self.cycle_space(dir)?,
            Cmd::AddSpace(space) => self.add_space(space)?,
            Cmd::EditSpace {
                id,
                name,
                icon,
                color,
            } => self.edit_space(&id, &name, &icon, color),
            Cmd::DeleteSpace { id } => self.delete_space(&id)?,
            Cmd::MoveTab { id, to, index } => self.move_tab(&id, &to, index),
            Cmd::CreateFolder { name } => self.create_folder(&name),
            Cmd::RenameFolder { id, name } => self.rename_folder(&id, &name),
            Cmd::DeleteFolder { id } => self.delete_folder(&id),
            Cmd::ToggleFolderCollapsed { id } => self.toggle_folder_collapsed(&id),
            Cmd::MoveFolder { id, to, index } => self.move_folder(&id, &to, index),
            Cmd::MoveTabToFolder {
                id,
                folder_id,
                index,
            } => self.move_tab_to_folder(&id, folder_id, index),
            Cmd::UpdateAccept => self.answer_update(true),
            Cmd::UpdateLater => self.answer_update(false),
            Cmd::ListHistory { query } => self.list_history(&query),
            Cmd::DeleteHistory { id } => self.delete_history_entry(&id),
            Cmd::ClearHistory => self.clear_history(),
            Cmd::SetTheme { theme } => {
                if matches!(theme.as_str(), "system" | "light" | "dark") {
                    self.p.theme = theme;
                    self.apply_theme();
                    self.save();
                }
            }
            Cmd::SetRestoreTabs { on } => {
                self.p.restore_tabs = on;
                self.save();
            }
            Cmd::SetAutoPip { on } => {
                self.p.auto_pip = on;
                self.save();
            }
            Cmd::UpdateProfileCleanup(hours) => self.update_profile_cleanup(hours),
            Cmd::ListExt => self.push_exts(),
            Cmd::InstallExt { input } => match store::parse_id(&input) {
                Some(id) => {
                    self.toast("Installing extension…");
                    self.spawn_install(id);
                }
                None => self.toast("That doesn’t look like a Web Store link or extension ID."),
            },
            Cmd::RemoveExt { dir } => match store::remove(&self.root, &dir) {
                Ok(()) => {
                    self.toast("Removed. Restart the browser to finish unloading it.");
                    self.push_exts();
                }
                Err(e) => self.toast(&format!("Couldn’t remove: {e}")),
            },
            Cmd::Back => self.eval_visible("history.back()"),
            Cmd::Forward => self.eval_visible("history.forward()"),
            Cmd::Reload => self.eval_visible("location.reload()"),
            Cmd::DevToolsToggle => self.toggle_devtools()?,
            Cmd::DevToolsPosition { position } => {
                if matches!(position.as_str(), "right" | "left" | "bottom" | "top") {
                    self.p.devtools_position = position;
                    self.p.devtools_size = self.devtools_size();
                    self.save();
                    self.relayout();
                }
            }
            Cmd::DevToolsSize { size, persist } => {
                if size.is_finite() {
                    self.p.devtools_size = size.clamp(PANEL_MIN, self.devtools_limit());
                    if persist {
                        self.save();
                    }
                    self.relayout();
                }
            }
            Cmd::DevToolsIcons { icons } => self.set_devtools_icons(icons),
            Cmd::SwitcherPoll => {
                // Ctrl may be released before the sidebar has keyboard focus, so poll for it.
                if self.switcher.is_some() && !ctrl_held() {
                    self.switcher_close(true)?;
                }
            }
            Cmd::SwitcherCommit => self.switcher_close(true)?,
            Cmd::SwitcherCancel => self.switcher_close(false)?,
            Cmd::SwitcherPick { id } => {
                if let Some(sw) = self.switcher.as_mut() {
                    if let Some(i) = sw.ids.iter().position(|x| *x == id) {
                        sw.index = i;
                    }
                }
                self.switcher_close(true)?;
            }
        }
        Ok(())
    }
}
