use crate::{
    candidates,
    ipc::IpcCommand,
    models::{Config, CustomApp, DomainRule, FavoriteEntry, HistoryEntry, LaunchRequest},
    storage::{self, Store},
    windows_integration::{self, RegistrationState},
};
use floem::{
    Clipboard, IntoView, Urgency, WindowIdExt,
    ext_event::create_signal_from_channel,
    keyboard::Key,
    peniko::{Color, kurbo::Size},
    prelude::*,
    reactive::{RwSignal, SignalGet, SignalUpdate, create_effect},
    views::{
        button, dyn_stack, dyn_view, h_stack, label, labeled_checkbox, scroll, text, text_input,
        v_stack,
    },
    window::{WindowConfig, WindowId, close_window, new_window},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    History,
    Favorites,
    Registration,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    MainWindow,
    InterceptWindow,
}

impl AppMode {
    pub fn window_title(self) -> &'static str {
        match self {
            Self::MainWindow => "Link Interceptor",
            Self::InterceptWindow => "拦截 URL",
        }
    }

    pub fn initial_size(self) -> Size {
        match self {
            Self::MainWindow => Size::new(1024.0, 720.0),
            Self::InterceptWindow => Size::new(860.0, 560.0),
        }
    }
}

#[derive(Clone)]
pub struct LinkInterceptorApp {
    store: Option<Store>,
    config: RwSignal<Config>,
    history: RwSignal<Vec<HistoryEntry>>,
    favorites: RwSignal<Vec<FavoriteEntry>>,
    active_tab: RwSignal<Tab>,
    history_query: RwSignal<String>,
    favorites_query: RwSignal<String>,
    status: RwSignal<String>,
    registration_state: RwSignal<RegistrationState>,
    main_window: RwSignal<Option<WindowId>>,
    next_intercept_id: RwSignal<u64>,
    clear_history_confirmation: RwSignal<bool>,
    reset_config_confirmation: RwSignal<bool>,
    new_custom_name: RwSignal<String>,
    new_custom_executable: RwSignal<String>,
    new_custom_args: RwSignal<String>,
    new_domain_pattern: RwSignal<String>,
    new_domain_app_name: RwSignal<String>,
}

impl LinkInterceptorApp {
    pub fn new(launch_request: Option<LaunchRequest>) -> Self {
        let store = Store::new().ok();
        let config = store
            .as_ref()
            .and_then(|store| store.load_config().ok())
            .unwrap_or_default();

        let mut history = store
            .as_ref()
            .and_then(|store| store.load_history().ok())
            .unwrap_or_default();
        let favorites = store
            .as_ref()
            .and_then(|store| store.load_favorites().ok())
            .unwrap_or_default();

        if let Some(request) = launch_request.as_ref() {
            if !request.raw_url.trim().is_empty() {
                storage::record_history(&mut history, request.raw_url.trim());
                if let Some(store) = &store {
                    let _ = store.save_history(&history);
                }
            }
        }

        let new_custom_app = CustomApp::default();
        let new_domain_rule = DomainRule::default();
        Self {
            store,
            config: RwSignal::new(config),
            history: RwSignal::new(history),
            favorites: RwSignal::new(favorites),
            active_tab: RwSignal::new(Tab::History),
            history_query: RwSignal::new(String::new()),
            favorites_query: RwSignal::new(String::new()),
            status: RwSignal::new(String::new()),
            registration_state: RwSignal::new(windows_integration::registration_state()),
            main_window: RwSignal::new(None),
            next_intercept_id: RwSignal::new(1),
            clear_history_confirmation: RwSignal::new(false),
            reset_config_confirmation: RwSignal::new(false),
            new_custom_name: RwSignal::new(new_custom_app.name),
            new_custom_executable: RwSignal::new(new_custom_app.executable),
            new_custom_args: RwSignal::new(new_custom_app.args_template),
            new_domain_pattern: RwSignal::new(new_domain_rule.pattern),
            new_domain_app_name: RwSignal::new(new_domain_rule.app_name),
        }
    }

    pub fn run(
        launch_request: Option<LaunchRequest>,
        ipc_receiver: Option<std::sync::mpsc::Receiver<IpcCommand>>,
    ) {
        let initial_url = launch_request
            .as_ref()
            .map(|request| request.raw_url.clone());
        let app = Self::new(launch_request);
        app.start_ipc_bridge(ipc_receiver);

        let mode = if initial_url.is_some() {
            AppMode::InterceptWindow
        } else {
            AppMode::MainWindow
        };
        if initial_url.is_some() {
            app.next_intercept_id.set(2);
        }

        let app_for_window = app.clone();
        let config = if initial_url.is_some() {
            window_config(mode).title("拦截 URL #1")
        } else {
            window_config(mode)
        };
        floem::Application::new()
            .window(
                move |window_id| {
                    if let Some(url) = initial_url.clone() {
                        app_for_window.intercept_window_view(window_id, url)
                    } else {
                        app_for_window.main_window.set(Some(window_id));
                        app_for_window.main_window_view(window_id)
                    }
                },
                Some(config),
            )
            .run();
    }

    fn start_ipc_bridge(&self, receiver: Option<std::sync::mpsc::Receiver<IpcCommand>>) {
        let Some(receiver) = receiver else {
            return;
        };
        let (sender, floem_receiver) = crossbeam_channel::unbounded();
        std::thread::spawn(move || {
            while let Ok(command) = receiver.recv() {
                let _ = sender.send(command);
            }
        });

        let command_signal = create_signal_from_channel(floem_receiver);
        let app = self.clone();
        create_effect(move |_| {
            if let Some(command) = command_signal.get() {
                match command {
                    IpcCommand::ShowMain => app.show_or_create_main_window(),
                    IpcCommand::OpenIntercept { url } => app.open_intercept_window(url, true),
                }
            }
        });
    }

    fn persist_all(&self) {
        if let Some(store) = &self.store {
            if let Err(error) = store.save_config(&self.config.get()) {
                self.status.set(format!("保存配置失败：{error}"));
                return;
            }
            if let Err(error) = store.save_history(&self.history.get()) {
                self.status.set(format!("保存历史记录失败：{error}"));
                return;
            }
            if let Err(error) = store.save_favorites(&self.favorites.get()) {
                self.status.set(format!("保存收藏失败：{error}"));
                return;
            }
            self.status.set("已保存".to_owned());
        } else {
            self.status.set("存储目录不可用".to_owned());
        }
    }

    fn persist_config(&self) {
        if let Some(store) = &self.store {
            match store.save_config(&self.config.get()) {
                Ok(()) => self.status.set("配置已保存".to_owned()),
                Err(error) => self.status.set(format!("保存配置失败：{error}")),
            }
        } else {
            self.status.set("存储目录不可用".to_owned());
        }
    }

    fn persist_history(&self) {
        if let Some(store) = &self.store {
            let _ = store.save_history(&self.history.get());
        }
    }

    fn persist_favorites(&self) {
        if let Some(store) = &self.store {
            let _ = store.save_favorites(&self.favorites.get());
        }
    }

    fn record_history(&self, url: &str) {
        let url = url.trim().to_owned();
        if url.is_empty() {
            return;
        }
        self.history
            .update(|history| storage::record_history(history, &url));
        self.persist_history();
    }

    fn toggle_favorite_url(&self, url: &str) -> String {
        let url = url.trim();
        if url.is_empty() {
            return "没有可收藏的 URL".to_owned();
        }
        let mut added = false;
        self.favorites.update(|favorites| {
            added = storage::toggle_favorite(favorites, url);
        });
        self.persist_favorites();
        if added {
            "已添加到收藏".to_owned()
        } else {
            "已从收藏移除".to_owned()
        }
    }

    fn open_candidate_for_url(
        &self,
        candidate: &crate::models::OpenCandidate,
        url: &str,
    ) -> String {
        let url = url.trim();
        if url.is_empty() {
            return "没有可打开的 URL".to_owned();
        }
        self.record_history(url);
        match windows_integration::launch_candidate(candidate, url) {
            Ok(()) => format!("已通过 {} 打开", candidate.name),
            Err(error) => format!("{} 打开失败：{error}", candidate.name),
        }
    }

    fn show_or_create_main_window(&self) {
        if let Some(window_id) = self.main_window.get() {
            bring_window_to_front(window_id);
            return;
        }

        let app = self.clone();
        new_window(
            move |window_id| {
                app.main_window.set(Some(window_id));
                if app.config.get().bring_new_windows_to_front {
                    bring_window_to_front(window_id);
                }
                app.main_window_view(window_id)
            },
            Some(window_config(AppMode::MainWindow)),
        );
    }

    fn open_intercept_window(&self, url: String, record_history: bool) {
        let url = url.trim().to_owned();
        if record_history {
            self.record_history(&url);
        }

        let app = self.clone();
        let id = self.next_intercept_id.get();
        self.next_intercept_id.set(id + 1);
        new_window(
            move |window_id| {
                if app.config.get().bring_new_windows_to_front {
                    bring_window_to_front(window_id);
                }
                app.intercept_window_view(window_id, url.clone())
            },
            Some(
                window_config(AppMode::InterceptWindow)
                    .title(format!("拦截 URL #{id}"))
                    .size(AppMode::InterceptWindow.initial_size()),
            ),
        );
    }

    fn main_window_view(&self, window_id: WindowId) -> floem::AnyView {
        let app = self.clone();
        let tab = self.active_tab;
        let content_app = self.clone();
        v_stack((
            h_stack((
                tab_button("历史记录", tab, Tab::History),
                tab_button("收藏", tab, Tab::Favorites),
                tab_button("注册状态", tab, Tab::Registration),
                tab_button("设置", tab, Tab::Settings),
            ))
            .style(|s| s.gap(8).padding(10)),
            dyn_view(move || match tab.get() {
                Tab::History => content_app.history_view().into_any(),
                Tab::Favorites => content_app.favorites_view().into_any(),
                Tab::Registration => content_app.registration_view().into_any(),
                Tab::Settings => content_app.settings_view().into_any(),
            })
            .style(|s| {
                s.flex_grow(1.0)
                    .flex_shrink(1.0)
                    .width_full()
                    .min_height(0.0)
            }),
            label(move || {
                let status = app.status.get();
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
                if app.main_window.get() == Some(window_id) {
                    app.main_window.set(None);
                }
            }
        })
        .style(|s| s.size_full().flex_col())
        .into_any()
    }

    fn intercept_window_view(&self, window_id: WindowId, initial_url: String) -> floem::AnyView {
        let url = RwSignal::new(initial_url);
        let window_status = RwSignal::new(String::new());
        let app = self.clone();
        let candidates_app = self.clone();
        v_stack((
            text("拦截到的 URL").style(|s| s.font_size(22.0)),
            text_input(url)
                .style(|s| s.width_full().min_height(72.0))
                .keyboard_navigable(),
            h_stack((
                button("复制").action({
                    let window_status = window_status;
                    move || match Clipboard::set_contents(url.get()) {
                        Ok(()) => window_status.set("已复制 URL".to_owned()),
                        Err(error) => window_status.set(format!("复制失败：{error:?}")),
                    }
                }),
                button(label(move || {
                    if storage::is_favorite(&app.favorites.get(), url.get().trim()) {
                        "取消收藏".to_owned()
                    } else {
                        "收藏".to_owned()
                    }
                }))
                .action({
                    let app = app.clone();
                    let window_status = window_status;
                    move || window_status.set(app.toggle_favorite_url(&url.get()))
                }),
                button("保存到历史记录").action({
                    let app = app.clone();
                    let window_status = window_status;
                    move || {
                        app.record_history(&url.get());
                        window_status.set("已保存到历史记录".to_owned());
                    }
                }),
            ))
            .style(|s| s.gap(8)),
            text("打开方式").style(|s| s.font_size(20.0)),
            scroll(
                dyn_stack(
                    move || candidates::build_candidates(&candidates_app.config.get(), &url.get()),
                    |candidate| {
                        (
                            candidate.name.clone(),
                            format!("{:?}", candidate.kind),
                            candidate.command.clone(),
                        )
                    },
                    {
                        let app = self.clone();
                        move |candidate: crate::models::OpenCandidate| {
                            let name = candidate.name.clone();
                            let enabled = candidate.available;
                            h_stack((
                                button(name).disabled(move || !enabled).action({
                                    let app = app.clone();
                                    let candidate = candidate.clone();
                                    let window_status = window_status;
                                    move || {
                                        window_status.set(
                                            app.open_candidate_for_url(&candidate, &url.get()),
                                        );
                                    }
                                }),
                                text(candidate_kind_label(candidate.kind).to_owned()),
                                text(candidate.reason),
                            ))
                            .style(|s| s.gap(8).items_center())
                        }
                    },
                )
                .style(|s| s.flex_col().gap(6)),
            )
            .style(|s| s.flex_grow(1.0).width_full()),
            label(move || {
                let status = window_status.get();
                if status.is_empty() {
                    "就绪".to_owned()
                } else {
                    status
                }
            }),
        ))
        .on_key_down(
            Key::Character("w".into()),
            |modifiers| modifiers.control(),
            move |_| {
                close_window(window_id);
            },
        )
        .style(|s| s.size_full().padding(14).gap(10).flex_col())
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
                                app.history.set(Vec::new());
                                app.persist_history();
                                app.clear_history_confirmation.set(false);
                                app.status.set("历史记录已清空".to_owned());
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
                                            app.history.update(|history| {
                                                history.retain(|item| item.url != url);
                                            });
                                            app.persist_history();
                                        }
                                    })
                                    .style(|s| s.flex_shrink(0.0)),
                                button("打开")
                                    .action({
                                        let app = app.clone();
                                        let url = url.clone();
                                        move || app.open_intercept_window(url.clone(), false)
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
                                            app.favorites.update(|favorites| {
                                                favorites.retain(|item| item.url != url);
                                            });
                                            app.persist_favorites();
                                        }
                                    })
                                    .style(|s| s.flex_shrink(0.0)),
                                button("打开")
                                    .action({
                                        let app = app.clone();
                                        let url = url.clone();
                                        move || app.open_intercept_window(url.clone(), false)
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
                app.registration_state
                    .set(windows_integration::registration_state())
            }
        });

        v_stack((
            text("注册状态").style(|s| s.font_size(22.0)),
            label({
                let app = app.clone();
                move || match app.registration_state.get() {
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
                            app.registration_state
                                .set(windows_integration::registration_state());
                            app.status
                                .set("已注册。请在 Windows 设置中将其设为默认应用。".to_owned());
                        }
                        Err(error) => app.status.set(format!("注册失败：{error}")),
                    }
                }),
                button("反注册").action({
                    let app = app.clone();
                    move || match windows_integration::unregister_application() {
                        Ok(()) => {
                            app.registration_state
                                .set(windows_integration::registration_state());
                            app.status.set("已反注册".to_owned());
                        }
                        Err(error) => app.status.set(format!("反注册失败：{error}")),
                    }
                }),
                button("打开默认应用设置").action({
                    let app = app.clone();
                    move || match windows_integration::open_default_apps_settings() {
                        Ok(()) => app.status.set("已打开 Windows 设置".to_owned()),
                        Err(error) => app.status.set(format!("打开设置失败：{error}")),
                    }
                }),
            ))
            .style(|s| s.gap(8)),
        ))
        .style(|s| s.size_full().padding(14).gap(10).flex_col())
    }

    fn settings_view(&self) -> impl IntoView + 'static {
        let app = self.clone();
        v_stack((
            text("窗口").style(|s| s.font_size(22.0)),
            labeled_checkbox(
                move || app.config.get().bring_new_windows_to_front,
                || "打开新窗口时自动置顶",
            )
            .on_update({
                let app = self.clone();
                move |checked| {
                    app.config.update(|config| {
                        config.bring_new_windows_to_front = checked;
                    });
                    app.persist_config();
                }
            }),
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
                        app.config.update(|config| {
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
                        app.persist_config();
                    }
                }),
            ))
            .style(|s| s.gap(6).items_center()),
            text("域名规则").style(|s| s.font_size(20.0)),
            self.domain_rules_view(),
            text("添加域名规则").style(|s| s.font_size(16.0)),
            h_stack((
                text("匹配模式"),
                text_input(self.new_domain_pattern).style(|s| s.width(180.0)),
                text("应用名称"),
                text_input(self.new_domain_app_name).style(|s| s.width(180.0)),
                button("添加").action({
                    let app = self.clone();
                    move || {
                        let pattern = app.new_domain_pattern.get();
                        if pattern.trim().is_empty() {
                            return;
                        }
                        app.config.update(|config| {
                            config.domain_rules.push(DomainRule {
                                pattern,
                                app_name: app.new_domain_app_name.get(),
                            });
                        });
                        let default = DomainRule::default();
                        app.new_domain_pattern.set(default.pattern);
                        app.new_domain_app_name.set(default.app_name);
                        app.persist_config();
                    }
                }),
            ))
            .style(|s| s.gap(6).items_center()),
            h_stack((
                button("保存设置").action({
                    let app = self.clone();
                    move || app.persist_all()
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
                                    app.config.set(Config::default());
                                    let default_app = CustomApp::default();
                                    let default_rule = DomainRule::default();
                                    app.new_custom_name.set(default_app.name);
                                    app.new_custom_executable.set(default_app.executable);
                                    app.new_custom_args.set(default_app.args_template);
                                    app.new_domain_pattern.set(default_rule.pattern);
                                    app.new_domain_app_name.set(default_rule.app_name);
                                    app.persist_config();
                                    app.reset_config_confirmation.set(false);
                                    app.status.set("已恢复默认设置".to_owned());
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
        .style(|s| s.size_full().padding(14).gap(10).flex_col())
    }

    fn custom_apps_view(&self) -> impl IntoView + 'static {
        let app = self.clone();
        dyn_stack(
            move || {
                app.config
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
                                state.config.update(|config| {
                                    if let Some(app) = config.custom_apps.get_mut(index) {
                                        app.name = name.get();
                                        app.executable = executable.get();
                                        app.args_template = args_template.get();
                                    }
                                });
                                state.persist_config();
                            }
                        }),
                        button("移除").action({
                            let state = state.clone();
                            move || {
                                state.config.update(|config| {
                                    if index < config.custom_apps.len() {
                                        config.custom_apps.remove(index);
                                    }
                                });
                                state.persist_config();
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
                app.config
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
                        text("匹配模式"),
                        text_input(pattern).style(|s| s.width(180.0)),
                        text("应用"),
                        text_input(app_name).style(|s| s.width(180.0)),
                        button("保存").action({
                            let state = state.clone();
                            move || {
                                state.config.update(|config| {
                                    if let Some(rule) = config.domain_rules.get_mut(index) {
                                        rule.pattern = pattern.get();
                                        rule.app_name = app_name.get();
                                    }
                                });
                                state.persist_config();
                            }
                        }),
                        button("移除").action({
                            let state = state.clone();
                            move || {
                                state.config.update(|config| {
                                    if index < config.domain_rules.len() {
                                        config.domain_rules.remove(index);
                                    }
                                });
                                state.persist_config();
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

fn tab_button(label_text: &'static str, active_tab: RwSignal<Tab>, tab: Tab) -> impl IntoView {
    button(label(move || label_text.to_owned())).action(move || active_tab.set(tab))
}

fn window_config(mode: AppMode) -> WindowConfig {
    WindowConfig::default()
        .title(mode.window_title())
        .size(mode.initial_size())
}

fn bring_window_to_front(window_id: WindowId) {
    window_id.set_visible(true);
    window_id.minimized(false);
    window_id.request_attention(Urgency::Informational);
}

fn candidate_kind_label(kind: crate::models::CandidateKind) -> &'static str {
    match kind {
        crate::models::CandidateKind::Browser => "浏览器",
        crate::models::CandidateKind::ProtocolHandler => "协议处理程序",
        crate::models::CandidateKind::DomainApp => "域名应用",
        crate::models::CandidateKind::CustomApp => "自定义应用",
        crate::models::CandidateKind::ShellFallback => "Windows 默认处理程序",
    }
}
