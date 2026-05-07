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
                self.status = format!("Failed to save config: {error}");
                return;
            }
            if let Err(error) = store.save_history(&self.history) {
                self.status = format!("Failed to save history: {error}");
                return;
            }
            if let Err(error) = store.save_favorites(&self.favorites) {
                self.status = format!("Failed to save favorites: {error}");
                return;
            }
            self.status = "Saved".to_owned();
        } else {
            self.status = "Storage directory is unavailable".to_owned();
        }
    }

    fn persist_config(&mut self) {
        if let Some(store) = &self.store {
            match store.save_config(&self.config) {
                Ok(()) => self.status = "Config saved".to_owned(),
                Err(error) => self.status = format!("Failed to save config: {error}"),
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
            self.status = "No URL to favorite".to_owned();
            return;
        }
        let added = storage::toggle_favorite(&mut self.favorites, url);
        if let Some(store) = &self.store {
            let _ = store.save_favorites(&self.favorites);
        }
        self.status = if added {
            "Added to favorites".to_owned()
        } else {
            "Removed from favorites".to_owned()
        };
    }

    fn open_candidate(&mut self, candidate: &crate::models::OpenCandidate) {
        let url = self.current_url.trim();
        if url.is_empty() {
            self.status = "No URL to open".to_owned();
            return;
        }
        storage::record_history(&mut self.history, url);
        if let Some(store) = &self.store {
            let _ = store.save_history(&self.history);
        }
        match windows_integration::launch_candidate(candidate, url) {
            Ok(()) => self.status = format!("Opened with {}", candidate.name),
            Err(error) => self.status = format!("Failed to open with {}: {error}", candidate.name),
        }
    }
}

impl eframe::App for LinkInterceptorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                tab_button(ui, &mut self.active_tab, Tab::Intercept, "Intercept");
                tab_button(ui, &mut self.active_tab, Tab::History, "History");
                tab_button(ui, &mut self.active_tab, Tab::Favorites, "Favorites");
                tab_button(ui, &mut self.active_tab, Tab::Registration, "Registration");
                tab_button(ui, &mut self.active_tab, Tab::Settings, "Settings");
            });
        });

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(if self.status.is_empty() {
                    "Ready"
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
        ui.heading("Intercepted URL");
        ui.add_space(8.0);
        ui.add(
            egui::TextEdit::multiline(&mut self.current_url)
                .desired_rows(4)
                .lock_focus(true)
                .hint_text("URL or deeplink"),
        );
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Copy").clicked() {
                ctx.copy_text(self.current_url.clone());
                self.status = "Copied URL".to_owned();
            }
            let favorite_label = if storage::is_favorite(&self.favorites, self.current_url.trim()) {
                "Remove favorite"
            } else {
                "Favorite"
            };
            if ui.button(favorite_label).clicked() {
                self.toggle_current_favorite();
            }
            if ui.button("Save to history").clicked() {
                let url = self.current_url.trim().to_owned();
                if !url.is_empty() {
                    storage::record_history(&mut self.history, &url);
                    if let Some(store) = &self.store {
                        let _ = store.save_history(&self.history);
                    }
                    self.status = "Saved to history".to_owned();
                }
            }
        });

        ui.separator();
        ui.heading("Open With");
        let candidates = candidates::build_candidates(&self.config, self.current_url.trim());
        egui::ScrollArea::vertical().show(ui, |ui| {
            for candidate in candidates {
                ui.horizontal(|ui| {
                    let label = if candidate.is_primary {
                        format!("{}  ·  primary", candidate.name)
                    } else {
                        candidate.name.clone()
                    };
                    let button = ui.add_enabled(candidate.available, egui::Button::new(label));
                    if button.clicked() {
                        self.open_candidate(&candidate);
                    }
                    ui.label(format!("{:?}", candidate.kind));
                    ui.small(candidate.reason);
                });
            }
        });
    }

    fn ui_history(&mut self, ui: &mut egui::Ui) {
        ui.heading("History");
        ui.horizontal(|ui| {
            ui.label("Search");
            ui.text_edit_singleline(&mut self.history_query);
            if ui.button("Clear history").clicked() {
                self.history.clear();
                if let Some(store) = &self.store {
                    let _ = store.save_history(&self.history);
                }
                self.status = "History cleared".to_owned();
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
                    if ui.button("Use").clicked() {
                        self.set_current_url(entry.url.clone());
                    }
                    if ui.button("Delete").clicked() {
                        self.history.retain(|item| item.url != entry.url);
                        if let Some(store) = &self.store {
                            let _ = store.save_history(&self.history);
                        }
                    }
                    ui.vertical(|ui| {
                        ui.label(&entry.url);
                        ui.small(format!(
                            "Last: {} · Count: {}",
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
        ui.heading("Favorites");
        ui.horizontal(|ui| {
            ui.label("Search");
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
                    if ui.button("Use").clicked() {
                        self.set_current_url(entry.url.clone());
                    }
                    if ui.button("Remove").clicked() {
                        self.favorites.retain(|item| item.url != entry.url);
                        if let Some(store) = &self.store {
                            let _ = store.save_favorites(&self.favorites);
                        }
                    }
                    ui.vertical(|ui| {
                        ui.label(&entry.url);
                        ui.small(format!(
                            "Added: {}",
                            entry.added_at.format("%Y-%m-%d %H:%M:%S")
                        ));
                    });
                });
                ui.separator();
            }
        });
    }

    fn ui_registration(&mut self, ui: &mut egui::Ui) {
        ui.heading("Registration");
        self.registration_state = windows_integration::registration_state();
        ui.label(match self.registration_state {
            RegistrationState::NotRegistered => "State: not registered as a browser candidate",
            RegistrationState::Registered => {
                "State: registered, but Windows may not use it by default"
            }
            RegistrationState::PossibleDefault => {
                "State: registered and possibly selected as default"
            }
        });
        if let Ok(exe) = windows_integration::current_exe() {
            ui.small(format!("Current exe: {}", exe.display()));
        }
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Register current exe").clicked() {
                match windows_integration::register_application() {
                    Ok(()) => {
                        self.status =
                            "Registered. Set it as default in Windows settings.".to_owned()
                    }
                    Err(error) => self.status = format!("Registration failed: {error}"),
                }
            }
            if ui.button("Unregister").clicked() {
                match windows_integration::unregister_application() {
                    Ok(()) => self.status = "Unregistered".to_owned(),
                    Err(error) => self.status = format!("Unregister failed: {error}"),
                }
            }
            if ui.button("Open default apps settings").clicked() {
                match windows_integration::open_default_apps_settings() {
                    Ok(()) => self.status = "Opened Windows settings".to_owned(),
                    Err(error) => self.status = format!("Failed to open settings: {error}"),
                }
            }
        });
    }

    fn ui_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Custom Applications");
        let mut remove_app = None;
        for (index, app) in self.config.custom_apps.iter_mut().enumerate() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut app.name);
                    if ui.button("Remove").clicked() {
                        remove_app = Some(index);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Executable");
                    ui.text_edit_singleline(&mut app.executable);
                });
                ui.horizontal(|ui| {
                    ui.label("Args");
                    ui.text_edit_singleline(&mut app.args_template);
                });
            });
        }
        if let Some(index) = remove_app {
            self.config.custom_apps.remove(index);
            self.persist_config();
        }
        ui.collapsing("Add custom application", |ui| {
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut self.new_custom_app.name);
            });
            ui.horizontal(|ui| {
                ui.label("Executable");
                ui.text_edit_singleline(&mut self.new_custom_app.executable);
            });
            ui.horizontal(|ui| {
                ui.label("Args");
                ui.text_edit_singleline(&mut self.new_custom_app.args_template);
            });
            if ui.button("Add").clicked() {
                if !self.new_custom_app.name.trim().is_empty() {
                    self.config.custom_apps.push(self.new_custom_app.clone());
                    self.new_custom_app = CustomApp::default();
                    self.persist_config();
                }
            }
        });

        ui.separator();
        ui.heading("Domain Rules");
        let mut remove_rule = None;
        for (index, rule) in self.config.domain_rules.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label("Pattern");
                ui.text_edit_singleline(&mut rule.pattern);
                ui.label("App");
                ui.text_edit_singleline(&mut rule.app_name);
                if ui.button("Remove").clicked() {
                    remove_rule = Some(index);
                }
            });
        }
        if let Some(index) = remove_rule {
            self.config.domain_rules.remove(index);
            self.persist_config();
        }
        ui.collapsing("Add domain rule", |ui| {
            ui.horizontal(|ui| {
                ui.label("Pattern");
                ui.text_edit_singleline(&mut self.new_domain_rule.pattern);
            });
            ui.horizontal(|ui| {
                ui.label("App name");
                ui.text_edit_singleline(&mut self.new_domain_rule.app_name);
            });
            if ui.button("Add").clicked() {
                if !self.new_domain_rule.pattern.trim().is_empty() {
                    self.config.domain_rules.push(self.new_domain_rule.clone());
                    self.new_domain_rule = DomainRule::default();
                    self.persist_config();
                }
            }
        });

        ui.separator();
        if ui.button("Save settings").clicked() {
            self.persist_all();
        }
    }
}

fn tab_button(ui: &mut egui::Ui, active_tab: &mut Tab, tab: Tab, label: &str) {
    if ui.selectable_label(*active_tab == tab, label).clicked() {
        *active_tab = tab;
    }
}
