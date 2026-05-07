use crate::{
    candidates,
    ipc::IpcCommand,
    models::{Config, CustomApp, DomainRule, FavoriteEntry, HistoryEntry, LaunchRequest},
    storage::{self, Store},
    windows_integration::{self, RegistrationState},
};
use eframe::egui;
use std::sync::mpsc::Receiver;

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

    pub fn initial_size(self) -> [f32; 2] {
        match self {
            Self::MainWindow => [1024.0, 720.0],
            Self::InterceptWindow => [860.0, 560.0],
        }
    }

    pub fn min_size(self) -> [f32; 2] {
        match self {
            Self::MainWindow => [760.0, 520.0],
            Self::InterceptWindow => [640.0, 420.0],
        }
    }
}

pub struct LinkInterceptorApp {
    ipc_receiver: Option<Receiver<IpcCommand>>,
    next_intercept_id: u64,
    root_window: RootWindow,
    intercept_windows: Vec<InterceptWindow>,
    secondary_main_open: bool,
    store: Option<Store>,
    config: Config,
    history: Vec<HistoryEntry>,
    favorites: Vec<FavoriteEntry>,
    active_tab: Tab,
    history_query: String,
    favorites_query: String,
    status: String,
    registration_state: RegistrationState,
    new_custom_app: CustomApp,
    new_domain_rule: DomainRule,
}

enum RootWindow {
    Main,
    Intercept(InterceptWindow),
}

struct InterceptWindow {
    id: u64,
    url: String,
    status: String,
}

impl LinkInterceptorApp {
    pub fn new(
        launch_request: Option<LaunchRequest>,
        ipc_receiver: Option<Receiver<IpcCommand>>,
    ) -> Self {
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

        let initial_url = launch_request
            .as_ref()
            .map(|request| request.raw_url.clone())
            .unwrap_or_default();
        if !initial_url.is_empty() {
            storage::record_history(&mut history, &initial_url);
            if let Some(store) = &store {
                let _ = store.save_history(&history);
            }
        }
        let (root_window, next_intercept_id) = if initial_url.is_empty() {
            (RootWindow::Main, 1)
        } else {
            (
                RootWindow::Intercept(InterceptWindow {
                    id: 1,
                    url: initial_url,
                    status: String::new(),
                }),
                2,
            )
        };

        Self {
            ipc_receiver,
            next_intercept_id,
            root_window,
            intercept_windows: Vec::new(),
            secondary_main_open: false,
            store,
            config,
            history,
            favorites,
            active_tab: Tab::History,
            history_query: String::new(),
            favorites_query: String::new(),
            status: String::new(),
            registration_state: windows_integration::registration_state(),
            new_custom_app: CustomApp::default(),
            new_domain_rule: DomainRule::default(),
        }
    }

    fn persist_all(&mut self) {
        if let Some(store) = &self.store {
            if let Err(error) = store.save_config(&self.config) {
                self.status = format!("保存配置失败：{error}");
                return;
            }
            if let Err(error) = store.save_history(&self.history) {
                self.status = format!("保存历史记录失败：{error}");
                return;
            }
            if let Err(error) = store.save_favorites(&self.favorites) {
                self.status = format!("保存收藏失败：{error}");
                return;
            }
            self.status = "已保存".to_owned();
        } else {
            self.status = "存储目录不可用".to_owned();
        }
    }

    fn persist_config(&mut self) {
        if let Some(store) = &self.store {
            match store.save_config(&self.config) {
                Ok(()) => self.status = "配置已保存".to_owned(),
                Err(error) => self.status = format!("保存配置失败：{error}"),
            }
        }
    }

    fn open_intercept_window(&mut self, url: String, record_history: bool) {
        let url = url.trim().to_owned();
        if record_history && !url.is_empty() {
            storage::record_history(&mut self.history, &url);
            if let Some(store) = &self.store {
                let _ = store.save_history(&self.history);
            }
        }
        self.intercept_windows.push(InterceptWindow {
            id: self.next_intercept_id,
            url,
            status: String::new(),
        });
        self.next_intercept_id += 1;
    }

    fn toggle_favorite_url(&mut self, url: &str) -> String {
        let url = url.trim();
        if url.is_empty() {
            return "没有可收藏的 URL".to_owned();
        }
        let added = storage::toggle_favorite(&mut self.favorites, url);
        if let Some(store) = &self.store {
            let _ = store.save_favorites(&self.favorites);
        }
        if added {
            "已添加到收藏".to_owned()
        } else {
            "已从收藏移除".to_owned()
        }
    }

    fn open_candidate_for_url(
        &mut self,
        candidate: &crate::models::OpenCandidate,
        url: &str,
    ) -> String {
        let url = url.trim();
        if url.is_empty() {
            return "没有可打开的 URL".to_owned();
        }
        storage::record_history(&mut self.history, url);
        if let Some(store) = &self.store {
            let _ = store.save_history(&self.history);
        }
        match windows_integration::launch_candidate(candidate, url) {
            Ok(()) => format!("已通过 {} 打开", candidate.name),
            Err(error) => format!("{} 打开失败：{error}", candidate.name),
        }
    }

    fn show_main_window(&mut self, ctx: &egui::Context) {
        match self.root_window {
            RootWindow::Main => {
                ctx.send_viewport_cmd_to(
                    egui::ViewportId::ROOT,
                    egui::ViewportCommand::Minimized(false),
                );
                ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Focus);
                ctx.send_viewport_cmd_to(
                    egui::ViewportId::ROOT,
                    egui::ViewportCommand::RequestUserAttention(
                        egui::UserAttentionType::Informational,
                    ),
                );
            }
            RootWindow::Intercept(_) => {
                self.secondary_main_open = true;
                let viewport_id = egui::ViewportId::from_hash_of("main-window");
                ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Focus);
                ctx.send_viewport_cmd_to(
                    viewport_id,
                    egui::ViewportCommand::RequestUserAttention(
                        egui::UserAttentionType::Informational,
                    ),
                );
            }
        }
    }

    fn drain_ipc(&mut self, ctx: &egui::Context) {
        let mut commands = Vec::new();
        if let Some(receiver) = &self.ipc_receiver {
            while let Ok(command) = receiver.try_recv() {
                commands.push(command);
            }
        }
        for command in commands {
            match command {
                IpcCommand::ShowMain => self.show_main_window(ctx),
                IpcCommand::OpenIntercept { url } => self.open_intercept_window(url, true),
            }
        }
    }
}

impl eframe::App for LinkInterceptorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_ipc(ctx);
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
        self.show_secondary_main_window(ctx);
        self.show_intercept_windows(ctx);
        self.show_root_window(ctx);
    }
}

impl LinkInterceptorApp {
    fn show_root_window(&mut self, ctx: &egui::Context) {
        let root_window = std::mem::replace(&mut self.root_window, RootWindow::Main);
        match root_window {
            RootWindow::Main => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(
                    AppMode::MainWindow.window_title().to_owned(),
                ));
                self.ui_main(ctx);
                self.root_window = RootWindow::Main;
            }
            RootWindow::Intercept(mut window) => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                    "拦截 URL #{}",
                    window.id
                )));
                let close_requested = ctx.input(|input| input.viewport().close_requested());
                egui::TopBottomPanel::bottom("root_intercept_status").show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(if window.status.is_empty() {
                            "就绪"
                        } else {
                            &window.status
                        });
                    });
                });
                egui::CentralPanel::default().show(ctx, |ui| {
                    self.ui_intercept(ctx, ui, &mut window);
                });

                if close_requested {
                    self.replace_closed_root(ctx);
                } else {
                    self.root_window = RootWindow::Intercept(window);
                }
            }
        }
    }

    fn replace_closed_root(&mut self, ctx: &egui::Context) {
        if self.secondary_main_open {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.secondary_main_open = false;
            self.root_window = RootWindow::Main;
            return;
        }
        if let Some(window) = self.intercept_windows.pop() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.root_window = RootWindow::Intercept(window);
            return;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn show_secondary_main_window(&mut self, ctx: &egui::Context) {
        if !self.secondary_main_open {
            return;
        }

        let viewport_id = egui::ViewportId::from_hash_of("main-window");
        let builder = egui::ViewportBuilder::default()
            .with_title(AppMode::MainWindow.window_title())
            .with_inner_size(AppMode::MainWindow.initial_size())
            .with_min_inner_size(AppMode::MainWindow.min_size())
            .with_active(true);
        let close_requested = ctx.show_viewport_immediate(viewport_id, builder, |ctx, _class| {
            let close_requested = ctx.input(|input| input.viewport().close_requested());
            self.ui_main(ctx);
            close_requested
        });
        if close_requested {
            self.secondary_main_open = false;
        }
    }

    fn ui_main(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                tab_button(ui, &mut self.active_tab, Tab::History, "历史记录");
                tab_button(ui, &mut self.active_tab, Tab::Favorites, "收藏");
                tab_button(ui, &mut self.active_tab, Tab::Registration, "注册状态");
                tab_button(ui, &mut self.active_tab, Tab::Settings, "设置");
            });
        });

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(if self.status.is_empty() {
                    "就绪"
                } else {
                    &self.status
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.active_tab {
            Tab::History => self.ui_history(ui),
            Tab::Favorites => self.ui_favorites(ui),
            Tab::Registration => self.ui_registration(ui),
            Tab::Settings => self.ui_settings(ui),
        });
    }

    fn show_intercept_windows(&mut self, ctx: &egui::Context) {
        let mut open_windows = Vec::new();
        for mut window in std::mem::take(&mut self.intercept_windows) {
            let viewport_id = egui::ViewportId::from_hash_of(("intercept", window.id));
            let builder = egui::ViewportBuilder::default()
                .with_title(format!("拦截 URL #{}", window.id))
                .with_inner_size(AppMode::InterceptWindow.initial_size())
                .with_min_inner_size(AppMode::InterceptWindow.min_size())
                .with_active(true);
            let close_requested =
                ctx.show_viewport_immediate(viewport_id, builder, |ctx, _class| {
                    let close_requested = ctx.input(|input| input.viewport().close_requested());
                    egui::TopBottomPanel::bottom(format!("intercept_status_{}", window.id)).show(
                        ctx,
                        |ui| {
                            ui.horizontal(|ui| {
                                ui.label(if window.status.is_empty() {
                                    "就绪"
                                } else {
                                    &window.status
                                });
                            });
                        },
                    );

                    egui::CentralPanel::default().show(ctx, |ui| {
                        self.ui_intercept(ctx, ui, &mut window);
                    });
                    close_requested
                });
            if !close_requested {
                open_windows.push(window);
            }
        }
        self.intercept_windows = open_windows;
    }

    fn ui_intercept(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        window: &mut InterceptWindow,
    ) {
        ui.heading("拦截到的 URL");
        ui.add_space(8.0);
        ui.add(
            egui::TextEdit::multiline(&mut window.url)
                .desired_rows(4)
                .lock_focus(true)
                .hint_text("URL 或 deeplink"),
        );
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("复制").clicked() {
                ctx.copy_text(window.url.clone());
                window.status = "已复制 URL".to_owned();
            }
            let favorite_label = if storage::is_favorite(&self.favorites, window.url.trim()) {
                "取消收藏"
            } else {
                "收藏"
            };
            if ui.button(favorite_label).clicked() {
                window.status = self.toggle_favorite_url(&window.url);
            }
            if ui.button("保存到历史记录").clicked() {
                let url = window.url.trim().to_owned();
                if !url.is_empty() {
                    storage::record_history(&mut self.history, &url);
                    if let Some(store) = &self.store {
                        let _ = store.save_history(&self.history);
                    }
                    window.status = "已保存到历史记录".to_owned();
                }
            }
        });

        ui.separator();
        ui.heading("打开方式");
        let candidates = candidates::build_candidates(&self.config, window.url.trim());
        egui::ScrollArea::vertical().show(ui, |ui| {
            for candidate in candidates {
                ui.horizontal(|ui| {
                    let button =
                        ui.add_enabled(candidate.available, egui::Button::new(&candidate.name));
                    if button.clicked() {
                        window.status = self.open_candidate_for_url(&candidate, &window.url);
                    }
                    ui.label(candidate_kind_label(candidate.kind.clone()));
                    ui.small(candidate.reason);
                });
            }
        });
    }

    fn ui_history(&mut self, ui: &mut egui::Ui) {
        ui.heading("历史记录");
        ui.horizontal(|ui| {
            ui.label("搜索");
            ui.text_edit_singleline(&mut self.history_query);
            if ui.button("清空历史记录").clicked() {
                self.history.clear();
                if let Some(store) = &self.store {
                    let _ = store.save_history(&self.history);
                }
                self.status = "历史记录已清空".to_owned();
            }
        });
        ui.separator();
        let query = self.history_query.to_ascii_lowercase();
        let rows: Vec<HistoryEntry> = self
            .history
            .iter()
            .filter(|entry| query.is_empty() || entry.url.to_ascii_lowercase().contains(&query))
            .cloned()
            .collect();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for entry in rows {
                ui.horizontal(|ui| {
                    if ui.button("删除").clicked() {
                        self.history.retain(|item| item.url != entry.url);
                        if let Some(store) = &self.store {
                            let _ = store.save_history(&self.history);
                        }
                    }
                    if ui.button("打开").clicked() {
                        self.open_intercept_window(entry.url.clone(), false);
                    }
                    ui.vertical(|ui| {
                        ui.label(&entry.url);
                        ui.small(format!(
                            "最近：{} · 次数：{}",
                            entry.last_seen_at.format("%Y-%m-%d %H:%M:%S"),
                            entry.open_count
                        ));
                    });
                });
                ui.separator();
            }
        });
    }

    fn ui_favorites(&mut self, ui: &mut egui::Ui) {
        ui.heading("收藏");
        ui.horizontal(|ui| {
            ui.label("搜索");
            ui.text_edit_singleline(&mut self.favorites_query);
        });
        ui.separator();
        let query = self.favorites_query.to_ascii_lowercase();
        let rows: Vec<FavoriteEntry> = self
            .favorites
            .iter()
            .filter(|entry| query.is_empty() || entry.url.to_ascii_lowercase().contains(&query))
            .cloned()
            .collect();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for entry in rows {
                ui.horizontal(|ui| {
                    if ui.button("移除").clicked() {
                        self.favorites.retain(|item| item.url != entry.url);
                        if let Some(store) = &self.store {
                            let _ = store.save_favorites(&self.favorites);
                        }
                    }
                    if ui.button("打开").clicked() {
                        self.open_intercept_window(entry.url.clone(), false);
                    }
                    ui.vertical(|ui| {
                        ui.label(&entry.url);
                        ui.small(format!(
                            "添加时间：{}",
                            entry.added_at.format("%Y-%m-%d %H:%M:%S")
                        ));
                    });
                });
                ui.separator();
            }
        });
    }

    fn ui_registration(&mut self, ui: &mut egui::Ui) {
        ui.heading("注册状态");
        self.registration_state = windows_integration::registration_state();
        ui.label(match self.registration_state {
            RegistrationState::NotRegistered => "状态：尚未注册为浏览器候选项",
            RegistrationState::Registered => "状态：已注册，但 Windows 可能尚未将其设为默认",
            RegistrationState::PossibleDefault => "状态：已注册，并且可能已被选为默认应用",
        });
        if let Ok(exe) = windows_integration::current_exe() {
            ui.small(format!("当前 exe：{}", exe.display()));
        }
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("注册当前 exe").clicked() {
                match windows_integration::register_application() {
                    Ok(()) => {
                        self.status = "已注册。请在 Windows 设置中将其设为默认应用。".to_owned()
                    }
                    Err(error) => self.status = format!("注册失败：{error}"),
                }
            }
            if ui.button("反注册").clicked() {
                match windows_integration::unregister_application() {
                    Ok(()) => self.status = "已反注册".to_owned(),
                    Err(error) => self.status = format!("反注册失败：{error}"),
                }
            }
            if ui.button("打开默认应用设置").clicked() {
                match windows_integration::open_default_apps_settings() {
                    Ok(()) => self.status = "已打开 Windows 设置".to_owned(),
                    Err(error) => self.status = format!("打开设置失败：{error}"),
                }
            }
        });
    }

    fn ui_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("自定义应用");
        let mut remove_app = None;
        for (index, app) in self.config.custom_apps.iter_mut().enumerate() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("名称");
                    ui.text_edit_singleline(&mut app.name);
                    if ui.button("移除").clicked() {
                        remove_app = Some(index);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("可执行文件");
                    ui.text_edit_singleline(&mut app.executable);
                });
                ui.horizontal(|ui| {
                    ui.label("参数");
                    ui.text_edit_singleline(&mut app.args_template);
                });
            });
        }
        if let Some(index) = remove_app {
            self.config.custom_apps.remove(index);
            self.persist_config();
        }
        ui.collapsing("添加自定义应用", |ui| {
            ui.horizontal(|ui| {
                ui.label("名称");
                ui.text_edit_singleline(&mut self.new_custom_app.name);
            });
            ui.horizontal(|ui| {
                ui.label("可执行文件");
                ui.text_edit_singleline(&mut self.new_custom_app.executable);
            });
            ui.horizontal(|ui| {
                ui.label("参数");
                ui.text_edit_singleline(&mut self.new_custom_app.args_template);
            });
            if ui.button("添加").clicked() {
                if !self.new_custom_app.name.trim().is_empty() {
                    self.config.custom_apps.push(self.new_custom_app.clone());
                    self.new_custom_app = CustomApp::default();
                    self.persist_config();
                }
            }
        });

        ui.separator();
        ui.heading("域名规则");
        let mut remove_rule = None;
        for (index, rule) in self.config.domain_rules.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label("匹配模式");
                ui.text_edit_singleline(&mut rule.pattern);
                ui.label("应用");
                ui.text_edit_singleline(&mut rule.app_name);
                if ui.button("移除").clicked() {
                    remove_rule = Some(index);
                }
            });
        }
        if let Some(index) = remove_rule {
            self.config.domain_rules.remove(index);
            self.persist_config();
        }
        ui.collapsing("添加域名规则", |ui| {
            ui.horizontal(|ui| {
                ui.label("匹配模式");
                ui.text_edit_singleline(&mut self.new_domain_rule.pattern);
            });
            ui.horizontal(|ui| {
                ui.label("应用名称");
                ui.text_edit_singleline(&mut self.new_domain_rule.app_name);
            });
            if ui.button("添加").clicked() {
                if !self.new_domain_rule.pattern.trim().is_empty() {
                    self.config.domain_rules.push(self.new_domain_rule.clone());
                    self.new_domain_rule = DomainRule::default();
                    self.persist_config();
                }
            }
        });

        ui.separator();
        if ui.button("保存设置").clicked() {
            self.persist_all();
        }
    }
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

fn tab_button(ui: &mut egui::Ui, active_tab: &mut Tab, tab: Tab, label: &str) {
    if ui.selectable_label(*active_tab == tab, label).clicked() {
        *active_tab = tab;
    }
}
