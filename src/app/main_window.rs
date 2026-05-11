use super::{opening_rules::OpeningRulesPanel, LinkInterceptorApp, MainTab};
use crate::{
    models::{Config, FavoriteEntry, HistoryEntry},
    ui_style::interactive_cursor_style,
    windows_integration::{self, RegistrationState},
};
use floem::{
    keyboard::Key,
    peniko::Color,
    prelude::*,
    reactive::{create_effect, RwSignal, SignalGet, SignalUpdate},
    views::{
        button, dyn_stack, dyn_view, h_stack, label, labeled_checkbox, scroll, text, text_input,
        v_stack,
    },
    window::{close_window, WindowId},
    IntoView,
};

#[derive(Clone)]
pub(super) struct MainWindow {
    app: LinkInterceptorApp,
    window_id: WindowId,
    active_tab: RwSignal<MainTab>,
    history_query: RwSignal<String>,
    favorites_query: RwSignal<String>,
    clear_history_confirmation: RwSignal<bool>,
    reset_config_confirmation: RwSignal<bool>,
    opening_rules: OpeningRulesPanel,
}

impl MainWindow {
    pub(super) fn new(app: LinkInterceptorApp, window_id: WindowId, initial_tab: MainTab) -> Self {
        let opening_rules = OpeningRulesPanel::new(app.clone());
        Self {
            app,
            window_id,
            active_tab: RwSignal::new(initial_tab),
            history_query: RwSignal::new(String::new()),
            favorites_query: RwSignal::new(String::new()),
            clear_history_confirmation: RwSignal::new(false),
            reset_config_confirmation: RwSignal::new(false),
            opening_rules,
        }
    }

    pub(super) fn view(self) -> floem::AnyView {
        let window_id = self.window_id;
        let app = self.clone();
        let tab = self.active_tab;
        let content_app = self.clone();
        create_effect({
            let app = self.app.clone();
            move |_| {
                if let Some(requested_tab) = app.main_tab_request.get() {
                    tab.set(requested_tab);
                    app.main_tab_request.set(None);
                }
            }
        });
        v_stack((
            h_stack((
                tab_button("历史记录", tab, MainTab::History),
                tab_button("收藏", tab, MainTab::Favorites),
                tab_button("注册状态", tab, MainTab::Registration),
                tab_button("设置", tab, MainTab::Settings),
            ))
            .style(|s| s.gap(8).padding(10)),
            dyn_view(move || match tab.get() {
                MainTab::History => content_app.history_view().into_any(),
                MainTab::Favorites => content_app.favorites_view().into_any(),
                MainTab::Registration => content_app.registration_view().into_any(),
                MainTab::Settings => content_app.settings_view().into_any(),
            })
            .style(|s| {
                s.flex_grow(1.0)
                    .flex_shrink(1.0)
                    .width_full()
                    .min_height(0.0)
            }),
            label(move || {
                let status = app.app.state.status.get();
                if status.is_empty() {
                    "就绪".to_owned()
                } else {
                    status
                }
            })
            .style(|s| s.padding(10)),
        ))
        .on_key_down(
            Key::Character("w".into()),
            |modifiers| modifiers.control(),
            move |_| {
                close_window(window_id);
            },
        )
        .on_cleanup({
            let app = self.clone();
            move || {
                if app.app.main_window.get() == Some(window_id) {
                    app.app.main_window.set(None);
                }
            }
        })
        .style(|s| interactive_cursor_style(s.size_full().flex_col()))
        .into_any()
    }

    fn history_view(&self) -> impl IntoView + 'static {
        let app = self.clone();
        let rows_app = self.clone();
        v_stack((
            text("历史记录").style(|s| s.font_size(22.0)),
            h_stack((
                text("搜索"),
                text_input(self.history_query).style(|s| s.width(320.0)),
                button("清空历史记录").action({
                    let app = self.clone();
                    move || app.clear_history_confirmation.set(true)
                }),
            ))
            .style(|s| s.gap(8).items_center()),
            dyn_view(move || {
                if app.clear_history_confirmation.get() {
                    h_stack((
                        text("此操作会删除全部历史记录，且无法撤销。"),
                        button("取消").action({
                            let app = app.clone();
                            move || app.clear_history_confirmation.set(false)
                        }),
                        button("确认清空").action({
                            let app = app.clone();
                            move || {
                                app.app.state.history.set(Vec::new());
                                app.app.persist_history();
                                app.clear_history_confirmation.set(false);
                                app.app.state.status.set("历史记录已清空".to_owned());
                            }
                        }),
                    ))
                    .style(|s| s.gap(8).padding(8))
                    .into_any()
                } else {
                    text("").into_any()
                }
            }),
            scroll(
                dyn_stack(
                    move || {
                        let query = rows_app.history_query.get().to_ascii_lowercase();
                        rows_app
                            .app
                            .state
                            .history
                            .get()
                            .into_iter()
                            .filter(|entry| {
                                query.is_empty() || entry.url.to_ascii_lowercase().contains(&query)
                            })
                            .collect::<Vec<_>>()
                    },
                    |entry| entry.url.clone(),
                    {
                        let app = self.clone();
                        move |entry: HistoryEntry| {
                            let url = entry.url.clone();
                            h_stack((
                                button("删除")
                                    .action({
                                        let app = app.clone();
                                        let url = url.clone();
                                        move || {
                                            app.app.state.history.update(|history| {
                                                history.retain(|item| item.url != url);
                                            });
                                            app.app.persist_history();
                                        }
                                    })
                                    .style(|s| s.flex_shrink(0.0)),
                                button("打开")
                                    .action({
                                        let app = app.clone();
                                        let url = url.clone();
                                        move || app.app.open_intercept_window(url.clone(), false)
                                    })
                                    .style(|s| s.flex_shrink(0.0)),
                                v_stack((
                                    text(entry.url).style(|s| {
                                        s.font_size(15.0)
                                            .width_full()
                                            .min_width(0.0)
                                            .flex_shrink(1.0)
                                    }),
                                    text(format!(
                                        "最近：{} · 次数：{}",
                                        entry.last_seen_at.format("%Y-%m-%d %H:%M:%S"),
                                        entry.open_count
                                    ))
                                    .style(|s| {
                                        s.font_size(11.0)
                                            .color(Color::rgb8(100, 100, 100))
                                            .width_full()
                                            .min_width(0.0)
                                    }),
                                ))
                                .style(|s| {
                                    s.flex_grow(1.0)
                                        .flex_shrink(1.0)
                                        .flex_basis(0.0)
                                        .width_full()
                                        .min_width(0.0)
                                }),
                            ))
                            .style(|s| {
                                s.gap(8)
                                    .items_start()
                                    .padding(4)
                                    .width_full()
                                    .min_width(0.0)
                            })
                        }
                    },
                )
                .style(|s| s.flex_col().gap(4).width_full().min_width(0.0)),
            )
            .style(|s| {
                s.flex_grow(1.0)
                    .flex_shrink(1.0)
                    .width_full()
                    .min_height(0.0)
            }),
        ))
        .style(|s| s.size_full().padding(14).gap(10).flex_col().min_height(0.0))
    }

    fn favorites_view(&self) -> impl IntoView + 'static {
        let rows_app = self.clone();
        v_stack((
            text("收藏").style(|s| s.font_size(22.0)),
            h_stack((
                text("搜索"),
                text_input(self.favorites_query).style(|s| s.width(320.0)),
            ))
            .style(|s| s.gap(8).items_center()),
            scroll(
                dyn_stack(
                    move || {
                        let query = rows_app.favorites_query.get().to_ascii_lowercase();
                        rows_app
                            .app
                            .state
                            .favorites
                            .get()
                            .into_iter()
                            .filter(|entry| {
                                query.is_empty() || entry.url.to_ascii_lowercase().contains(&query)
                            })
                            .collect::<Vec<_>>()
                    },
                    |entry| entry.url.clone(),
                    {
                        let app = self.clone();
                        move |entry: FavoriteEntry| {
                            let url = entry.url.clone();
                            h_stack((
                                button("移除")
                                    .action({
                                        let app = app.clone();
                                        let url = url.clone();
                                        move || {
                                            app.app.state.favorites.update(|favorites| {
                                                favorites.retain(|item| item.url != url);
                                            });
                                            app.app.persist_favorites();
                                        }
                                    })
                                    .style(|s| s.flex_shrink(0.0)),
                                button("打开")
                                    .action({
                                        let app = app.clone();
                                        let url = url.clone();
                                        move || app.app.open_intercept_window(url.clone(), false)
                                    })
                                    .style(|s| s.flex_shrink(0.0)),
                                v_stack((
                                    text(entry.url).style(|s| {
                                        s.font_size(15.0)
                                            .width_full()
                                            .min_width(0.0)
                                            .flex_shrink(1.0)
                                    }),
                                    text(format!(
                                        "添加时间：{}",
                                        entry.added_at.format("%Y-%m-%d %H:%M:%S")
                                    ))
                                    .style(|s| {
                                        s.font_size(11.0)
                                            .color(Color::rgb8(100, 100, 100))
                                            .width_full()
                                            .min_width(0.0)
                                    }),
                                ))
                                .style(|s| {
                                    s.flex_grow(1.0)
                                        .flex_shrink(1.0)
                                        .flex_basis(0.0)
                                        .width_full()
                                        .min_width(0.0)
                                }),
                            ))
                            .style(|s| {
                                s.gap(8)
                                    .items_start()
                                    .padding(4)
                                    .width_full()
                                    .min_width(0.0)
                            })
                        }
                    },
                )
                .style(|s| s.flex_col().gap(4).width_full().min_width(0.0)),
            )
            .style(|s| {
                s.flex_grow(1.0)
                    .flex_shrink(1.0)
                    .width_full()
                    .min_height(0.0)
            }),
        ))
        .style(|s| s.size_full().padding(14).gap(10).flex_col().min_height(0.0))
    }

    fn registration_view(&self) -> impl IntoView + 'static {
        let app = self.clone();
        create_effect({
            let app = app.clone();
            move |_| {
                app.app
                    .state
                    .registration_state
                    .set(windows_integration::registration_state())
            }
        });

        v_stack((
            text("注册状态").style(|s| s.font_size(22.0)),
            label({
                let app = app.clone();
                move || match app.app.state.registration_state.get() {
                    RegistrationState::NotRegistered => "状态：尚未注册为浏览器候选项".to_owned(),
                    RegistrationState::Registered => {
                        "状态：已注册，但 Windows 可能尚未将其设为默认".to_owned()
                    }
                    RegistrationState::PossibleDefault => {
                        "状态：已注册，并且可能已被选为默认应用".to_owned()
                    }
                }
            }),
            text(
                windows_integration::current_exe()
                    .map(|exe| format!("当前 exe：{}", exe.display()))
                    .unwrap_or_else(|error| format!("当前 exe：{error}")),
            )
            .style(|s| s.font_size(12.0)),
            h_stack((
                button("注册当前 exe").action({
                    let app = app.clone();
                    move || match windows_integration::register_application() {
                        Ok(()) => {
                            app.app
                                .state
                                .registration_state
                                .set(windows_integration::registration_state());
                            app.app
                                .state
                                .status
                                .set("已注册。请在 Windows 设置中将其设为默认应用。".to_owned());
                        }
                        Err(error) => app.app.state.status.set(format!("注册失败：{error}")),
                    }
                }),
                button("反注册").action({
                    let app = app.clone();
                    move || match windows_integration::unregister_application() {
                        Ok(()) => {
                            app.app
                                .state
                                .registration_state
                                .set(windows_integration::registration_state());
                            app.app.state.status.set("已反注册".to_owned());
                        }
                        Err(error) => app.app.state.status.set(format!("反注册失败：{error}")),
                    }
                }),
                button("打开默认应用设置").action({
                    let app = app.clone();
                    move || match windows_integration::open_default_apps_settings() {
                        Ok(()) => app.app.state.status.set("已打开 Windows 设置".to_owned()),
                        Err(error) => app.app.state.status.set(format!("打开设置失败：{error}")),
                    }
                }),
            ))
            .style(|s| s.gap(8)),
        ))
        .style(|s| s.size_full().padding(14).gap(10).flex_col())
    }

    fn settings_view(&self) -> impl IntoView + 'static {
        let app = self.clone();
        scroll(
            v_stack((
                text("窗口").style(|s| s.font_size(22.0)),
                labeled_checkbox(
                    move || app.app.state.config.get().bring_new_windows_to_front,
                    || "打开新窗口时自动置顶",
                )
                .on_update({
                    let app = self.clone();
                    move |checked| {
                        app.app.state.config.update(|config| {
                            config.bring_new_windows_to_front = checked;
                        });
                        app.app.persist_config();
                    }
                }),
                labeled_checkbox(
                    move || app.app.state.config.get().close_intercept_window_after_open,
                    || "拦截窗口默认在打开链接后自动关闭",
                )
                .on_update({
                    let app = self.clone();
                    move |checked| {
                        app.app.state.config.update(|config| {
                            config.close_intercept_window_after_open = checked;
                        });
                        app.app.persist_config();
                    }
                }),
                self.opening_rules.view(),
                h_stack((
                    button("保存设置").action({
                        let app = self.clone();
                        move || app.app.persist_all()
                    }),
                    button("恢复默认设置").action({
                        let app = self.clone();
                        move || app.reset_config_confirmation.set(true)
                    }),
                ))
                .style(|s| s.gap(8)),
                dyn_view({
                    let app = self.clone();
                    move || {
                        if app.reset_config_confirmation.get() {
                            h_stack((
                                text("此操作会将设置恢复为默认值，不会删除历史记录和收藏。"),
                                button("取消").action({
                                    let app = app.clone();
                                    move || app.reset_config_confirmation.set(false)
                                }),
                                button("确认恢复").action({
                                    let app = app.clone();
                                    move || {
                                        app.app.state.config.set(Config::default());
                                        app.opening_rules.reset();
                                        app.app.persist_config();
                                        app.reset_config_confirmation.set(false);
                                        app.app.state.status.set("已恢复默认设置".to_owned());
                                    }
                                }),
                            ))
                            .style(|s| s.gap(8).padding(8))
                            .into_any()
                        } else {
                            text("").into_any()
                        }
                    }
                }),
            ))
            .style(|s| s.width_full().padding(14).gap(10).flex_col()),
        )
        .style(|s| s.size_full())
    }
}

fn tab_button(
    label_text: &'static str,
    active_tab: RwSignal<MainTab>,
    tab: MainTab,
) -> impl IntoView {
    button(label(move || label_text.to_owned())).action(move || active_tab.set(tab))
}
