use crate::{
    candidates,
    models::{Config, CustomApp, DomainRule, FavoriteEntry, HistoryEntry, LaunchRequest},
    storage::{self, Store},
    windows_integration::{self, RegistrationState},
};
use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Intercept,
    History,
    Favorites,
    Registration,
    Settings,
}

pub struct LinkInterceptorApp {
    store: Option<Store>,
    config: Config,
    history: Vec<HistoryEntry>,
    favorites: Vec<FavoriteEntry>,
    current_url: String,
    active_tab: Tab,
    history_query: String,
    favorites_query: String,
    status: String,
    registration_state: RegistrationState,
    new_custom_app: CustomApp,
    new_domain_rule: DomainRule,
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

        let current_url = launch_request
            .as_ref()
            .map(|request| request.raw_url.clone())
            .unwrap_or_default();
        if !current_url.is_empty() {
            storage::record_history(&mut history, &current_url);
            if let Some(store) = &store {
                let _ = store.save_history(&history);
            }
        }

        Self {
            store,
            config,
            history,
            favorites,
            current_url,
            active_tab: Tab::Intercept,
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

    fn set_current_url(&mut self, url: String) {
        self.current_url = url;
        if !self.current_url.trim().is_empty() {
            storage::record_history(&mut self.history, self.current_url.trim());
            if let Some(store) = &self.store {
                let _ = store.save_history(&self.history);
            }
        }
        self.active_tab = Tab::Intercept;
    }

    fn toggle_current_favorite(&mut self) {
        let url = self.current_url.trim();
        if url.is_empty() {
            self.status = "没有可收藏的 URL".to_owned();
            return;
        }
        let added = storage::toggle_favorite(&mut self.favorites, url);
        if let Some(store) = &self.store {
            let _ = store.save_favorites(&self.favorites);
        }
        self.status = if added {
            "已添加到收藏".to_owned()
        } else {
            "已从收藏移除".to_owned()
        };
    }

    fn open_candidate(&mut self, candidate: &crate::models::OpenCandidate) {
        let url = self.current_url.trim();
        if url.is_empty() {
            self.status = "没有可打开的 URL".to_owned();
            return;
        }
        storage::record_history(&mut self.history, url);
        if let Some(store) = &self.store {
            let _ = store.save_history(&self.history);
        }
        match windows_integration::launch_candidate(candidate, url) {
            Ok(()) => self.status = format!("已使用 {} 打开", candidate.name),
            Err(error) => self.status = format!("使用 {} 打开失败：{error}", candidate.name),
        }
    }
}

impl eframe::App for LinkInterceptorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                tab_button(ui, &mut self.active_tab, Tab::Intercept, "拦截");
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
            Tab::Intercept => self.ui_intercept(ctx, ui),
            Tab::History => self.ui_history(ui),
            Tab::Favorites => self.ui_favorites(ui),
            Tab::Registration => self.ui_registration(ui),
            Tab::Settings => self.ui_settings(ui),
        });
    }
}

impl LinkInterceptorApp {
    fn ui_intercept(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.heading("拦截到的 URL");
        ui.add_space(8.0);
        ui.add(
            egui::TextEdit::multiline(&mut self.current_url)
                .desired_rows(4)
                .lock_focus(true)
                .hint_text("URL 或 deeplink"),
        );
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("复制").clicked() {
                ctx.copy_text(self.current_url.clone());
                self.status = "已复制 URL".to_owned();
            }
            let favorite_label = if storage::is_favorite(&self.favorites, self.current_url.trim()) {
                "取消收藏"
            } else {
                "收藏"
            };
            if ui.button(favorite_label).clicked() {
                self.toggle_current_favorite();
            }
            if ui.button("保存到历史记录").clicked() {
                let url = self.current_url.trim().to_owned();
                if !url.is_empty() {
                    storage::record_history(&mut self.history, &url);
                    if let Some(store) = &self.store {
                        let _ = store.save_history(&self.history);
                    }
                    self.status = "已保存到历史记录".to_owned();
                }
            }
        });

        ui.separator();
        ui.heading("打开方式");
        let candidates = candidates::build_candidates(&self.config, self.current_url.trim());
        egui::ScrollArea::vertical().show(ui, |ui| {
            for candidate in candidates {
                ui.horizontal(|ui| {
                    let label = if candidate.is_primary {
                        format!("{}  ·  推荐", candidate.name)
                    } else {
                        candidate.name.clone()
                    };
                    let button = ui.add_enabled(candidate.available, egui::Button::new(label));
                    if button.clicked() {
                        self.open_candidate(&candidate);
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
                    if ui.button("使用").clicked() {
                        self.set_current_url(entry.url.clone());
                    }
                    if ui.button("删除").clicked() {
                        self.history.retain(|item| item.url != entry.url);
                        if let Some(store) = &self.store {
                            let _ = store.save_history(&self.history);
                        }
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
                    if ui.button("使用").clicked() {
                        self.set_current_url(entry.url.clone());
                    }
                    if ui.button("移除").clicked() {
                        self.favorites.retain(|item| item.url != entry.url);
                        if let Some(store) = &self.store {
                            let _ = store.save_favorites(&self.favorites);
                        }
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
