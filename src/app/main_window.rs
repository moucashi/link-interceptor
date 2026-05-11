use super::{LinkInterceptorApp, MainTab};
use crate::{
    models::{Config, CustomApp, DomainRule, FavoriteEntry, HistoryEntry, ProtocolRule},
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
    new_custom_name: RwSignal<String>,
    new_custom_executable: RwSignal<String>,
    new_custom_args: RwSignal<String>,
    new_domain_pattern: RwSignal<String>,
    new_domain_app_name: RwSignal<String>,
    new_protocol_scheme: RwSignal<String>,
    new_protocol_app_name: RwSignal<String>,
}

impl MainWindow {
    pub(super) fn new(app: LinkInterceptorApp, window_id: WindowId, initial_tab: MainTab) -> Self {
        let new_custom_app = CustomApp::default();
        let new_domain_rule = DomainRule::default();
        let new_protocol_rule = ProtocolRule::default();
        Self {
            app,
            window_id,
            active_tab: RwSignal::new(initial_tab),
            history_query: RwSignal::new(String::new()),
            favorites_query: RwSignal::new(String::new()),
            clear_history_confirmation: RwSignal::new(false),
            reset_config_confirmation: RwSignal::new(false),
            new_custom_name: RwSignal::new(new_custom_app.name),
            new_custom_executable: RwSignal::new(new_custom_app.executable),
            new_custom_args: RwSignal::new(new_custom_app.args_template),
            new_domain_pattern: RwSignal::new(new_domain_rule.pattern),
            new_domain_app_name: RwSignal::new(new_domain_rule.app_name),
            new_protocol_scheme: RwSignal::new(new_protocol_rule.scheme),
            new_protocol_app_name: RwSignal::new(new_protocol_rule.app_name),
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
                self.opening_rules_view(),
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
                                        let default_app = CustomApp::default();
                                        let default_rule = DomainRule::default();
                                        let default_protocol_rule = ProtocolRule::default();
                                        app.new_custom_name.set(default_app.name);
                                        app.new_custom_executable.set(default_app.executable);
                                        app.new_custom_args.set(default_app.args_template);
                                        app.new_domain_pattern.set(default_rule.pattern);
                                        app.new_domain_app_name.set(default_rule.app_name);
                                        app.new_protocol_scheme.set(default_protocol_rule.scheme);
                                        app.new_protocol_app_name
                                            .set(default_protocol_rule.app_name);
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

    fn opening_rules_view(&self) -> impl IntoView + 'static {
        v_stack((
            text("自定义应用").style(|s| s.font_size(20.0)),
            self.custom_apps_view(),
            text("添加自定义应用").style(|s| s.font_size(16.0)),
            h_stack((
                text("名称"),
                text_input(self.new_custom_name).style(|s| s.width(150.0)),
                text("可执行文件"),
                text_input(self.new_custom_executable).style(|s| s.width(240.0)),
                text("参数"),
                text_input(self.new_custom_args).style(|s| s.width(180.0)),
                button("添加").action({
                    let app = self.clone();
                    move || {
                        let name = app.new_custom_name.get();
                        if name.trim().is_empty() {
                            return;
                        }
                        app.app.state.config.update(|config| {
                            config.custom_apps.push(CustomApp {
                                name,
                                executable: app.new_custom_executable.get(),
                                args_template: app.new_custom_args.get(),
                            });
                        });
                        let default = CustomApp::default();
                        app.new_custom_name.set(default.name);
                        app.new_custom_executable.set(default.executable);
                        app.new_custom_args.set(default.args_template);
                        app.app.persist_config();
                    }
                }),
            ))
            .style(|s| s.gap(6).items_center()),
            text("域名规则").style(|s| s.font_size(20.0)),
            self.domain_rules_view(),
            text("添加域名规则").style(|s| s.font_size(16.0)),
            h_stack((
                text("根域名"),
                text_input(self.new_domain_pattern).style(|s| s.width(180.0)),
                text("自定义应用"),
                text_input(self.new_domain_app_name).style(|s| s.width(180.0)),
                button("添加").action({
                    let app = self.clone();
                    move || {
                        let pattern = app.new_domain_pattern.get();
                        if pattern.trim().is_empty() {
                            return;
                        }
                        app.app.state.config.update(|config| {
                            config.domain_rules.push(DomainRule {
                                pattern,
                                app_name: app.new_domain_app_name.get(),
                            });
                        });
                        let default = DomainRule::default();
                        app.new_domain_pattern.set(default.pattern);
                        app.new_domain_app_name.set(default.app_name);
                        app.app.persist_config();
                    }
                }),
            ))
            .style(|s| s.gap(6).items_center()),
            text("协议规则").style(|s| s.font_size(20.0)),
            self.protocol_rules_view(),
            text("添加协议规则").style(|s| s.font_size(16.0)),
            h_stack((
                text("协议"),
                text_input(self.new_protocol_scheme).style(|s| s.width(120.0)),
                text("自定义应用"),
                text_input(self.new_protocol_app_name).style(|s| s.width(180.0)),
                button("添加").action({
                    let app = self.clone();
                    move || {
                        let scheme = app.new_protocol_scheme.get();
                        if scheme.trim().is_empty() {
                            return;
                        }
                        app.app.state.config.update(|config| {
                            config.protocol_rules.push(ProtocolRule {
                                scheme,
                                app_name: app.new_protocol_app_name.get(),
                            });
                        });
                        let default = ProtocolRule::default();
                        app.new_protocol_scheme.set(default.scheme);
                        app.new_protocol_app_name.set(default.app_name);
                        app.app.persist_config();
                    }
                }),
            ))
            .style(|s| s.gap(6).items_center()),
        ))
        .style(|s| s.gap(10).flex_col())
    }

    fn custom_apps_view(&self) -> impl IntoView + 'static {
        let app = self.clone();
        dyn_stack(
            move || {
                app.app
                    .state
                    .config
                    .get()
                    .custom_apps
                    .into_iter()
                    .enumerate()
                    .collect::<Vec<_>>()
            },
            |(index, app)| (*index, app.name.clone()),
            {
                let state = self.clone();
                move |(index, custom_app): (usize, CustomApp)| {
                    let name = RwSignal::new(custom_app.name);
                    let executable = RwSignal::new(custom_app.executable);
                    let args_template = RwSignal::new(custom_app.args_template);
                    h_stack((
                        text("名称"),
                        text_input(name).style(|s| s.width(140.0)),
                        text("可执行文件"),
                        text_input(executable).style(|s| s.width(220.0)),
                        text("参数"),
                        text_input(args_template).style(|s| s.width(160.0)),
                        button("保存").action({
                            let state = state.clone();
                            move || {
                                state.app.state.config.update(|config| {
                                    if let Some(app) = config.custom_apps.get_mut(index) {
                                        app.name = name.get();
                                        app.executable = executable.get();
                                        app.args_template = args_template.get();
                                    }
                                });
                                state.app.persist_config();
                            }
                        }),
                        button("移除").action({
                            let state = state.clone();
                            move || {
                                state.app.state.config.update(|config| {
                                    if index < config.custom_apps.len() {
                                        config.custom_apps.remove(index);
                                    }
                                });
                                state.app.persist_config();
                            }
                        }),
                    ))
                    .style(|s| s.gap(6).items_center().padding(4))
                }
            },
        )
        .style(|s| s.flex_col().gap(4))
    }

    fn domain_rules_view(&self) -> impl IntoView + 'static {
        let app = self.clone();
        dyn_stack(
            move || {
                app.app
                    .state
                    .config
                    .get()
                    .domain_rules
                    .into_iter()
                    .enumerate()
                    .collect::<Vec<_>>()
            },
            |(index, rule)| (*index, rule.pattern.clone(), rule.app_name.clone()),
            {
                let state = self.clone();
                move |(index, rule): (usize, DomainRule)| {
                    let pattern = RwSignal::new(rule.pattern);
                    let app_name = RwSignal::new(rule.app_name);
                    h_stack((
                        text("根域名"),
                        text_input(pattern).style(|s| s.width(180.0)),
                        text("自定义应用"),
                        text_input(app_name).style(|s| s.width(180.0)),
                        button("保存").action({
                            let state = state.clone();
                            move || {
                                state.app.state.config.update(|config| {
                                    if let Some(rule) = config.domain_rules.get_mut(index) {
                                        rule.pattern = pattern.get();
                                        rule.app_name = app_name.get();
                                    }
                                });
                                state.app.persist_config();
                            }
                        }),
                        button("移除").action({
                            let state = state.clone();
                            move || {
                                state.app.state.config.update(|config| {
                                    if index < config.domain_rules.len() {
                                        config.domain_rules.remove(index);
                                    }
                                });
                                state.app.persist_config();
                            }
                        }),
                    ))
                    .style(|s| s.gap(6).items_center().padding(4))
                }
            },
        )
        .style(|s| s.flex_col().gap(4))
    }

    fn protocol_rules_view(&self) -> impl IntoView + 'static {
        let app = self.clone();
        dyn_stack(
            move || {
                app.app
                    .state
                    .config
                    .get()
                    .protocol_rules
                    .into_iter()
                    .enumerate()
                    .collect::<Vec<_>>()
            },
            |(index, rule)| (*index, rule.scheme.clone(), rule.app_name.clone()),
            {
                let state = self.clone();
                move |(index, rule): (usize, ProtocolRule)| {
                    let scheme = RwSignal::new(rule.scheme);
                    let app_name = RwSignal::new(rule.app_name);
                    h_stack((
                        text("协议"),
                        text_input(scheme).style(|s| s.width(120.0)),
                        text("自定义应用"),
                        text_input(app_name).style(|s| s.width(180.0)),
                        button("保存").action({
                            let state = state.clone();
                            move || {
                                state.app.state.config.update(|config| {
                                    if let Some(rule) = config.protocol_rules.get_mut(index) {
                                        rule.scheme = scheme.get();
                                        rule.app_name = app_name.get();
                                    }
                                });
                                state.app.persist_config();
                            }
                        }),
                        button("移除").action({
                            let state = state.clone();
                            move || {
                                state.app.state.config.update(|config| {
                                    if index < config.protocol_rules.len() {
                                        config.protocol_rules.remove(index);
                                    }
                                });
                                state.app.persist_config();
                            }
                        }),
                    ))
                    .style(|s| s.gap(6).items_center().padding(4))
                }
            },
        )
        .style(|s| s.flex_col().gap(4))
    }
}

fn tab_button(
    label_text: &'static str,
    active_tab: RwSignal<MainTab>,
    tab: MainTab,
) -> impl IntoView {
    button(label(move || label_text.to_owned())).action(move || active_tab.set(tab))
}
