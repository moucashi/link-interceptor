use crate::{
    ipc::IpcCommand,
    models::{Config, FavoriteEntry, HistoryEntry, LaunchRequest},
    storage::{self, Store},
    windows_integration::{self, RegistrationState},
};
use floem::{
    Urgency, WindowIdExt,
    action::focus_window,
    ext_event::create_signal_from_channel,
    peniko::kurbo::Size,
    reactive::{RwSignal, SignalGet, SignalUpdate, create_effect, untrack},
    window::{WindowConfig, WindowId, new_window},
};

mod intercept_window;
mod main_window;

use intercept_window::InterceptWindow;
use main_window::MainWindow;

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
            Self::InterceptWindow => Size::new(640.0, 380.0),
        }
    }

    pub fn minimum_size(self) -> Size {
        match self {
            Self::MainWindow => Size::new(720.0, 520.0),
            Self::InterceptWindow => Size::new(460.0, 320.0),
        }
    }
}

#[derive(Clone)]
pub(super) struct AppState {
    store: Option<Store>,
    config: RwSignal<Config>,
    history: RwSignal<Vec<HistoryEntry>>,
    favorites: RwSignal<Vec<FavoriteEntry>>,
    status: RwSignal<String>,
    registration_state: RwSignal<RegistrationState>,
}

#[derive(Clone)]
pub struct LinkInterceptorApp {
    state: AppState,
    main_window: RwSignal<Option<WindowId>>,
    next_intercept_id: RwSignal<u64>,
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

        Self {
            state: AppState {
                store,
                config: RwSignal::new(config),
                history: RwSignal::new(history),
                favorites: RwSignal::new(favorites),
                status: RwSignal::new(String::new()),
                registration_state: RwSignal::new(windows_integration::registration_state()),
            },
            main_window: RwSignal::new(None),
            next_intercept_id: RwSignal::new(1),
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
        let initial_window_title = if initial_url.is_some() {
            "拦截 URL #1".to_owned()
        } else {
            mode.window_title().to_owned()
        };
        let config = window_config(mode).title(initial_window_title.clone());
        floem::Application::new()
            .window(
                move |window_id| {
                    if let Some(url) = initial_url.clone() {
                        InterceptWindow::new(app_for_window.clone(), window_id, url).view()
                    } else {
                        app_for_window.main_window.set(Some(window_id));
                        MainWindow::new(app_for_window.clone(), window_id).view()
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
                untrack(|| match command {
                    IpcCommand::ShowMain => app.show_or_create_main_window(),
                    IpcCommand::OpenIntercept { url } => app.open_intercept_window(url, true),
                });
            }
        });
    }

    fn persist_all(&self) {
        if let Some(store) = &self.state.store {
            if let Err(error) = store.save_config(&self.state.config.get()) {
                self.state.status.set(format!("保存配置失败：{error}"));
                return;
            }
            if let Err(error) = store.save_history(&self.state.history.get()) {
                self.state.status.set(format!("保存历史记录失败：{error}"));
                return;
            }
            if let Err(error) = store.save_favorites(&self.state.favorites.get()) {
                self.state.status.set(format!("保存收藏失败：{error}"));
                return;
            }
            self.state.status.set("已保存".to_owned());
        } else {
            self.state.status.set("存储目录不可用".to_owned());
        }
    }

    fn persist_config(&self) {
        if let Some(store) = &self.state.store {
            match store.save_config(&self.state.config.get()) {
                Ok(()) => self.state.status.set("配置已保存".to_owned()),
                Err(error) => self.state.status.set(format!("保存配置失败：{error}")),
            }
        } else {
            self.state.status.set("存储目录不可用".to_owned());
        }
    }

    fn persist_history(&self) {
        if let Some(store) = &self.state.store {
            let _ = store.save_history(&self.state.history.get());
        }
    }

    fn persist_favorites(&self) {
        if let Some(store) = &self.state.store {
            let _ = store.save_favorites(&self.state.favorites.get());
        }
    }

    fn record_history(&self, url: &str) {
        let url = url.trim().to_owned();
        if url.is_empty() {
            return;
        }
        self.state
            .history
            .update(|history| storage::record_history(history, &url));
        self.persist_history();
    }

    fn toggle_favorite_url(&self, url: &str) -> String {
        let url = url.trim();
        if url.is_empty() {
            return "没有可收藏的 URL".to_owned();
        }
        let mut added = false;
        self.state.favorites.update(|favorites| {
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
    ) -> (String, bool) {
        let url = url.trim();
        if url.is_empty() {
            return ("没有可打开的 URL".to_owned(), false);
        }
        self.record_history(url);
        match windows_integration::launch_candidate(candidate, url) {
            Ok(()) => (format!("已通过 {} 打开", candidate.name), true),
            Err(error) => (format!("{} 打开失败：{error}", candidate.name), false),
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
                if app.state.config.get().bring_new_windows_to_front {
                    bring_window_to_front(window_id);
                    focus_window();
                }
                MainWindow::new(app.clone(), window_id).view()
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
        let title = format!("拦截 URL #{id}");
        new_window(
            move |window_id| {
                if app.state.config.get().bring_new_windows_to_front {
                    bring_window_to_front(window_id);
                    focus_window();
                }
                InterceptWindow::new(app.clone(), window_id, url.clone()).view()
            },
            Some(
                window_config(AppMode::InterceptWindow)
                    .title(title)
                    .size(AppMode::InterceptWindow.initial_size()),
            ),
        );
    }
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
