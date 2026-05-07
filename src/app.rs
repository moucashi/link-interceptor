use crate::{
    candidates,
    ipc::IpcCommand,
    models::{
        CandidateKind, Config, CustomApp, DomainRule, FavoriteEntry, HistoryEntry, LaunchRequest,
        OpenCandidate,
    },
    storage::{self, Store},
    windows_integration::{self, RegistrationState},
};
use iced::{
    Element, Event, Length, Point, Size, Subscription, Task, clipboard, event, keyboard, mouse,
    time,
    widget::{
        Column, Row, Space, button, checkbox, container, responsive, rule, scrollable, stack, text,
        text_editor, text_input,
    },
    window,
};
use std::{collections::HashMap, sync::mpsc::Receiver, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
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

    pub fn min_size(self) -> Size {
        match self {
            Self::MainWindow => Size::new(760.0, 520.0),
            Self::InterceptWindow => Size::new(640.0, 420.0),
        }
    }
}

pub struct LinkInterceptorApp {
    ipc_receiver: Option<Receiver<IpcCommand>>,
    windows: HashMap<window::Id, WindowState>,
    main_window: Option<window::Id>,
    next_intercept_id: u64,
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
    last_cursor_positions: HashMap<window::Id, Point>,
    clear_history_confirmation: Option<Confirmation>,
    reset_config_confirmation: Option<Confirmation>,
}

enum WindowState {
    Main,
    Intercept(InterceptWindow),
}

struct InterceptWindow {
    id: u64,
    content: text_editor::Content,
    status: String,
}

impl InterceptWindow {
    fn new(id: u64, url: String) -> Self {
        Self {
            id,
            content: text_editor::Content::with_text(&url),
            status: String::new(),
        }
    }

    fn url(&self) -> String {
        self.content.text()
    }
}

#[derive(Debug, Clone, Copy)]
struct Confirmation {
    window_id: window::Id,
    target_pos: Point,
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Tick,
    RuntimeEvent(window::Id, Event),
    WindowOpened(window::Id),
    WindowCloseRequested(window::Id),
    WindowClosed(window::Id),
    SelectTab(Tab),
    HistoryQueryChanged(String),
    FavoritesQueryChanged(String),
    RequestClearHistory(window::Id),
    CancelClearHistory,
    ConfirmClearHistory,
    RequestResetConfig(window::Id),
    CancelResetConfig,
    ConfirmResetConfig,
    DeleteHistory(String),
    OpenHistory(String),
    RemoveFavorite(String),
    OpenFavorite(String),
    InterceptEdited(window::Id, text_editor::Action),
    CopyInterceptUrl(window::Id),
    ToggleInterceptFavorite(window::Id),
    SaveInterceptHistory(window::Id),
    OpenCandidate(window::Id, OpenCandidate),
    RegisterApplication,
    UnregisterApplication,
    OpenDefaultAppsSettings,
    BringNewWindowsToFrontChanged(bool),
    CustomAppNameChanged(usize, String),
    CustomAppExecutableChanged(usize, String),
    CustomAppArgsChanged(usize, String),
    RemoveCustomApp(usize),
    NewCustomAppNameChanged(String),
    NewCustomAppExecutableChanged(String),
    NewCustomAppArgsChanged(String),
    AddCustomApp,
    DomainRulePatternChanged(usize, String),
    DomainRuleAppNameChanged(usize, String),
    RemoveDomainRule(usize),
    NewDomainRulePatternChanged(String),
    NewDomainRuleAppNameChanged(String),
    AddDomainRule,
    SaveSettings,
}

impl LinkInterceptorApp {
    pub(crate) fn boot(
        launch_request: Option<LaunchRequest>,
        ipc_receiver: Option<Receiver<IpcCommand>>,
    ) -> (Self, Task<Message>) {
        let mut app = Self::new(ipc_receiver);
        let task = if let Some(request) = launch_request {
            app.open_intercept_window(request.raw_url, true)
        } else {
            app.open_main_window()
        };
        (app, task)
    }

    fn new(ipc_receiver: Option<Receiver<IpcCommand>>) -> Self {
        let store = Store::new().ok();
        let config = store
            .as_ref()
            .and_then(|store| store.load_config().ok())
            .unwrap_or_default();
        let history = store
            .as_ref()
            .and_then(|store| store.load_history().ok())
            .unwrap_or_default();
        let favorites = store
            .as_ref()
            .and_then(|store| store.load_favorites().ok())
            .unwrap_or_default();

        Self {
            ipc_receiver,
            windows: HashMap::new(),
            main_window: None,
            next_intercept_id: 1,
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
            last_cursor_positions: HashMap::new(),
            clear_history_confirmation: None,
            reset_config_confirmation: None,
        }
    }

    pub(crate) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => self.drain_ipc(),
            Message::RuntimeEvent(window_id, event) => self.handle_runtime_event(window_id, event),
            Message::WindowOpened(window_id) => {
                if self.config.bring_new_windows_to_front {
                    bring_window_to_front(window_id)
                } else {
                    Task::none()
                }
            }
            Message::WindowCloseRequested(window_id) => self.close_window(window_id),
            Message::WindowClosed(window_id) => {
                self.remove_window_state(window_id);
                if self.windows.is_empty() {
                    iced::exit()
                } else {
                    Task::none()
                }
            }
            Message::SelectTab(tab) => {
                self.active_tab = tab;
                if tab == Tab::Registration {
                    self.registration_state = windows_integration::registration_state();
                }
                Task::none()
            }
            Message::HistoryQueryChanged(value) => {
                self.history_query = value;
                Task::none()
            }
            Message::FavoritesQueryChanged(value) => {
                self.favorites_query = value;
                Task::none()
            }
            Message::RequestClearHistory(window_id) => {
                self.clear_history_confirmation = Some(Confirmation {
                    window_id,
                    target_pos: self.cursor_pos(window_id),
                });
                Task::none()
            }
            Message::CancelClearHistory => {
                self.clear_history_confirmation = None;
                Task::none()
            }
            Message::ConfirmClearHistory => {
                self.history.clear();
                if let Some(store) = &self.store {
                    let _ = store.save_history(&self.history);
                }
                self.clear_history_confirmation = None;
                self.status = "历史记录已清空".to_owned();
                Task::none()
            }
            Message::RequestResetConfig(window_id) => {
                self.reset_config_confirmation = Some(Confirmation {
                    window_id,
                    target_pos: self.cursor_pos(window_id),
                });
                Task::none()
            }
            Message::CancelResetConfig => {
                self.reset_config_confirmation = None;
                Task::none()
            }
            Message::ConfirmResetConfig => {
                self.config = Config::default();
                self.new_custom_app = CustomApp::default();
                self.new_domain_rule = DomainRule::default();
                if let Some(store) = &self.store {
                    match store.save_config(&self.config) {
                        Ok(()) => self.status = "已恢复默认设置".to_owned(),
                        Err(error) => self.status = format!("恢复默认设置失败：{error}"),
                    }
                } else {
                    self.status = "存储目录不可用".to_owned();
                }
                self.reset_config_confirmation = None;
                Task::none()
            }
            Message::DeleteHistory(url) => {
                self.history.retain(|entry| entry.url != url);
                if let Some(store) = &self.store {
                    let _ = store.save_history(&self.history);
                }
                Task::none()
            }
            Message::OpenHistory(url) => self.open_intercept_window(url, false),
            Message::RemoveFavorite(url) => {
                self.favorites.retain(|entry| entry.url != url);
                if let Some(store) = &self.store {
                    let _ = store.save_favorites(&self.favorites);
                }
                Task::none()
            }
            Message::OpenFavorite(url) => self.open_intercept_window(url, false),
            Message::InterceptEdited(window_id, action) => {
                if let Some(WindowState::Intercept(window)) = self.windows.get_mut(&window_id) {
                    window.content.perform(action);
                }
                Task::none()
            }
            Message::CopyInterceptUrl(window_id) => {
                let url = self.intercept_url(window_id);
                self.set_intercept_status(window_id, "已复制 URL".to_owned());
                clipboard::write(url)
            }
            Message::ToggleInterceptFavorite(window_id) => {
                let url = self.intercept_url(window_id);
                let status = self.toggle_favorite_url(&url);
                self.set_intercept_status(window_id, status);
                Task::none()
            }
            Message::SaveInterceptHistory(window_id) => {
                let url = self.intercept_url(window_id).trim().to_owned();
                if !url.is_empty() {
                    storage::record_history(&mut self.history, &url);
                    if let Some(store) = &self.store {
                        let _ = store.save_history(&self.history);
                    }
                    self.set_intercept_status(window_id, "已保存到历史记录".to_owned());
                }
                Task::none()
            }
            Message::OpenCandidate(window_id, candidate) => {
                let url = self.intercept_url(window_id);
                let status = self.open_candidate_for_url(&candidate, &url);
                self.set_intercept_status(window_id, status);
                Task::none()
            }
            Message::RegisterApplication => {
                match windows_integration::register_application() {
                    Ok(()) => {
                        self.status = "已注册。请在 Windows 设置中将其设为默认应用。".to_owned()
                    }
                    Err(error) => self.status = format!("注册失败：{error}"),
                }
                self.registration_state = windows_integration::registration_state();
                Task::none()
            }
            Message::UnregisterApplication => {
                match windows_integration::unregister_application() {
                    Ok(()) => self.status = "已反注册".to_owned(),
                    Err(error) => self.status = format!("反注册失败：{error}"),
                }
                self.registration_state = windows_integration::registration_state();
                Task::none()
            }
            Message::OpenDefaultAppsSettings => {
                match windows_integration::open_default_apps_settings() {
                    Ok(()) => self.status = "已打开 Windows 设置".to_owned(),
                    Err(error) => self.status = format!("打开设置失败：{error}"),
                }
                Task::none()
            }
            Message::BringNewWindowsToFrontChanged(value) => {
                self.config.bring_new_windows_to_front = value;
                self.persist_config();
                Task::none()
            }
            Message::CustomAppNameChanged(index, value) => {
                if let Some(app) = self.config.custom_apps.get_mut(index) {
                    app.name = value;
                }
                Task::none()
            }
            Message::CustomAppExecutableChanged(index, value) => {
                if let Some(app) = self.config.custom_apps.get_mut(index) {
                    app.executable = value;
                }
                Task::none()
            }
            Message::CustomAppArgsChanged(index, value) => {
                if let Some(app) = self.config.custom_apps.get_mut(index) {
                    app.args_template = value;
                }
                Task::none()
            }
            Message::RemoveCustomApp(index) => {
                if index < self.config.custom_apps.len() {
                    self.config.custom_apps.remove(index);
                    self.persist_config();
                }
                Task::none()
            }
            Message::NewCustomAppNameChanged(value) => {
                self.new_custom_app.name = value;
                Task::none()
            }
            Message::NewCustomAppExecutableChanged(value) => {
                self.new_custom_app.executable = value;
                Task::none()
            }
            Message::NewCustomAppArgsChanged(value) => {
                self.new_custom_app.args_template = value;
                Task::none()
            }
            Message::AddCustomApp => {
                if !self.new_custom_app.name.trim().is_empty() {
                    self.config.custom_apps.push(self.new_custom_app.clone());
                    self.new_custom_app = CustomApp::default();
                    self.persist_config();
                }
                Task::none()
            }
            Message::DomainRulePatternChanged(index, value) => {
                if let Some(rule) = self.config.domain_rules.get_mut(index) {
                    rule.pattern = value;
                }
                Task::none()
            }
            Message::DomainRuleAppNameChanged(index, value) => {
                if let Some(rule) = self.config.domain_rules.get_mut(index) {
                    rule.app_name = value;
                }
                Task::none()
            }
            Message::RemoveDomainRule(index) => {
                if index < self.config.domain_rules.len() {
                    self.config.domain_rules.remove(index);
                    self.persist_config();
                }
                Task::none()
            }
            Message::NewDomainRulePatternChanged(value) => {
                self.new_domain_rule.pattern = value;
                Task::none()
            }
            Message::NewDomainRuleAppNameChanged(value) => {
                self.new_domain_rule.app_name = value;
                Task::none()
            }
            Message::AddDomainRule => {
                if !self.new_domain_rule.pattern.trim().is_empty() {
                    self.config.domain_rules.push(self.new_domain_rule.clone());
                    self.new_domain_rule = DomainRule::default();
                    self.persist_config();
                }
                Task::none()
            }
            Message::SaveSettings => {
                self.persist_all();
                Task::none()
            }
        }
    }

    pub(crate) fn view(&self, window_id: window::Id) -> Element<'_, Message> {
        match self.windows.get(&window_id) {
            Some(WindowState::Main) => self.view_main(window_id),
            Some(WindowState::Intercept(window)) => self.view_intercept(window_id, window),
            None => container(text("窗口已关闭")).padding(16).into(),
        }
    }

    pub(crate) fn title(&self, window_id: window::Id) -> String {
        match self.windows.get(&window_id) {
            Some(WindowState::Main) => AppMode::MainWindow.window_title().to_owned(),
            Some(WindowState::Intercept(window)) => format!("拦截 URL #{}", window.id),
            None => "Link Interceptor".to_owned(),
        }
    }

    pub(crate) fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            time::every(Duration::from_millis(200)).map(|_| Message::Tick),
            window::close_requests().map(Message::WindowCloseRequested),
            window::close_events().map(Message::WindowClosed),
            event::listen_with(runtime_event)
                .map(|(window_id, event)| Message::RuntimeEvent(window_id, event)),
        ])
    }

    fn drain_ipc(&mut self) -> Task<Message> {
        let mut commands = Vec::new();
        if let Some(receiver) = &self.ipc_receiver {
            while let Ok(command) = receiver.try_recv() {
                commands.push(command);
            }
        }

        let mut tasks = Vec::new();
        for command in commands {
            match command {
                IpcCommand::ShowMain => tasks.push(self.open_main_window()),
                IpcCommand::OpenIntercept { url } => {
                    tasks.push(self.open_intercept_window(url, true));
                }
            }
        }
        Task::batch(tasks)
    }

    fn handle_runtime_event(&mut self, window_id: window::Id, event: Event) -> Task<Message> {
        match event {
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                self.last_cursor_positions.insert(window_id, position);
                Task::none()
            }
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. })
                if modifiers.control() && is_w_key(&key) =>
            {
                self.close_window(window_id)
            }
            _ => Task::none(),
        }
    }

    fn open_main_window(&mut self) -> Task<Message> {
        if let Some(window_id) = self.main_window {
            return bring_window_to_front(window_id);
        }

        let (window_id, task) = window::open(window_settings(AppMode::MainWindow));
        self.windows.insert(window_id, WindowState::Main);
        self.main_window = Some(window_id);
        task.map(Message::WindowOpened)
    }

    fn open_intercept_window(&mut self, url: String, record_history: bool) -> Task<Message> {
        let url = url.trim().to_owned();
        if record_history && !url.is_empty() {
            storage::record_history(&mut self.history, &url);
            if let Some(store) = &self.store {
                let _ = store.save_history(&self.history);
            }
        }

        let intercept_id = self.next_intercept_id;
        self.next_intercept_id += 1;
        let (window_id, task) = window::open(window_settings(AppMode::InterceptWindow));
        self.windows.insert(
            window_id,
            WindowState::Intercept(InterceptWindow::new(intercept_id, url)),
        );
        task.map(Message::WindowOpened)
    }

    fn close_window(&mut self, window_id: window::Id) -> Task<Message> {
        self.remove_window_state(window_id);
        if self.windows.is_empty() {
            Task::batch([window::close(window_id), iced::exit()])
        } else {
            window::close(window_id)
        }
    }

    fn remove_window_state(&mut self, window_id: window::Id) {
        if matches!(self.windows.remove(&window_id), Some(WindowState::Main)) {
            self.main_window = None;
        }
        self.last_cursor_positions.remove(&window_id);
        if self
            .clear_history_confirmation
            .is_some_and(|confirmation| confirmation.window_id == window_id)
        {
            self.clear_history_confirmation = None;
        }
        if self
            .reset_config_confirmation
            .is_some_and(|confirmation| confirmation.window_id == window_id)
        {
            self.reset_config_confirmation = None;
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
        } else {
            self.status = "存储目录不可用".to_owned();
        }
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

    fn open_candidate_for_url(&mut self, candidate: &OpenCandidate, url: &str) -> String {
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

    fn intercept_url(&self, window_id: window::Id) -> String {
        match self.windows.get(&window_id) {
            Some(WindowState::Intercept(window)) => window.url(),
            _ => String::new(),
        }
    }

    fn set_intercept_status(&mut self, window_id: window::Id, status: String) {
        if let Some(WindowState::Intercept(window)) = self.windows.get_mut(&window_id) {
            window.status = status;
        }
    }

    fn cursor_pos(&self, window_id: window::Id) -> Point {
        self.last_cursor_positions
            .get(&window_id)
            .copied()
            .unwrap_or(Point::new(120.0, 120.0))
    }

    fn view_main(&self, window_id: window::Id) -> Element<'_, Message> {
        let tabs = Row::new()
            .spacing(8)
            .push(tab_button(self.active_tab, Tab::History, "历史记录"))
            .push(tab_button(self.active_tab, Tab::Favorites, "收藏"))
            .push(tab_button(self.active_tab, Tab::Registration, "注册状态"))
            .push(tab_button(self.active_tab, Tab::Settings, "设置"));

        let content = match self.active_tab {
            Tab::History => self.view_history(window_id),
            Tab::Favorites => self.view_favorites(),
            Tab::Registration => self.view_registration(),
            Tab::Settings => self.view_settings(window_id),
        };

        let status = if self.status.is_empty() {
            "就绪".to_owned()
        } else {
            self.status.clone()
        };
        let base = container(
            Column::new()
                .spacing(12)
                .padding(16)
                .push(tabs)
                .push(rule::horizontal(1))
                .push(content)
                .push(rule::horizontal(1))
                .push(text(status)),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into();

        self.with_main_overlay(window_id, base)
    }

    fn with_main_overlay<'a>(
        &self,
        window_id: window::Id,
        base: Element<'a, Message>,
    ) -> Element<'a, Message> {
        let mut layers = vec![base];
        if let Some(confirmation) = self.clear_history_confirmation {
            if confirmation.window_id == window_id {
                layers.push(confirmation_overlay(
                    confirmation.target_pos,
                    "此操作会删除全部历史记录，且无法撤销。",
                    "确认清空",
                    Message::CancelClearHistory,
                    Message::ConfirmClearHistory,
                ));
            }
        }
        if let Some(confirmation) = self.reset_config_confirmation {
            if confirmation.window_id == window_id {
                layers.push(confirmation_overlay(
                    confirmation.target_pos,
                    "此操作会将设置恢复为默认值，不会删除历史记录和收藏。",
                    "确认恢复",
                    Message::CancelResetConfig,
                    Message::ConfirmResetConfig,
                ));
            }
        }
        stack(layers)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_history(&self, window_id: window::Id) -> Element<'_, Message> {
        let tools = Row::new()
            .spacing(8)
            .push(text("搜索"))
            .push(
                text_input("输入 URL 关键词", &self.history_query)
                    .on_input(Message::HistoryQueryChanged)
                    .width(Length::Fill),
            )
            .push(button("清空历史记录").on_press(Message::RequestClearHistory(window_id)));

        let query = self.history_query.to_ascii_lowercase();
        let mut rows = Column::new().spacing(8);
        for entry in self
            .history
            .iter()
            .filter(|entry| query.is_empty() || entry.url.to_ascii_lowercase().contains(&query))
        {
            let actions = Row::new()
                .spacing(8)
                .push(button("删除").on_press(Message::DeleteHistory(entry.url.clone())))
                .push(button("打开").on_press(Message::OpenHistory(entry.url.clone())));
            let details = Column::new()
                .spacing(4)
                .push(text(entry.url.clone()))
                .push(text(format!(
                    "最近：{} · 次数：{}",
                    entry.last_seen_at.format("%Y-%m-%d %H:%M:%S"),
                    entry.open_count
                )));
            rows = rows
                .push(Row::new().spacing(12).push(actions).push(details))
                .push(rule::horizontal(1));
        }

        Column::new()
            .spacing(12)
            .push(text("历史记录").size(24))
            .push(tools)
            .push(scrollable(rows).height(Length::Fill))
            .height(Length::Fill)
            .into()
    }

    fn view_favorites(&self) -> Element<'_, Message> {
        let tools = Row::new().spacing(8).push(text("搜索")).push(
            text_input("输入 URL 关键词", &self.favorites_query)
                .on_input(Message::FavoritesQueryChanged)
                .width(Length::Fill),
        );

        let query = self.favorites_query.to_ascii_lowercase();
        let mut rows = Column::new().spacing(8);
        for entry in self
            .favorites
            .iter()
            .filter(|entry| query.is_empty() || entry.url.to_ascii_lowercase().contains(&query))
        {
            let actions = Row::new()
                .spacing(8)
                .push(button("移除").on_press(Message::RemoveFavorite(entry.url.clone())))
                .push(button("打开").on_press(Message::OpenFavorite(entry.url.clone())));
            let details = Column::new()
                .spacing(4)
                .push(text(entry.url.clone()))
                .push(text(format!(
                    "添加时间：{}",
                    entry.added_at.format("%Y-%m-%d %H:%M:%S")
                )));
            rows = rows
                .push(Row::new().spacing(12).push(actions).push(details))
                .push(rule::horizontal(1));
        }

        Column::new()
            .spacing(12)
            .push(text("收藏").size(24))
            .push(tools)
            .push(scrollable(rows).height(Length::Fill))
            .height(Length::Fill)
            .into()
    }

    fn view_registration(&self) -> Element<'_, Message> {
        let state = match self.registration_state {
            RegistrationState::NotRegistered => "状态：尚未注册为浏览器候选项",
            RegistrationState::Registered => "状态：已注册，但 Windows 可能尚未将其设为默认",
            RegistrationState::PossibleDefault => "状态：已注册，并且可能已被选为默认应用",
        };
        let exe = windows_integration::current_exe()
            .map(|exe| format!("当前 exe：{}", exe.display()))
            .unwrap_or_else(|error| format!("当前 exe：{error}"));

        Column::new()
            .spacing(12)
            .push(text("注册状态").size(24))
            .push(text(state))
            .push(text(exe))
            .push(
                Row::new()
                    .spacing(8)
                    .push(button("注册当前 exe").on_press(Message::RegisterApplication))
                    .push(button("反注册").on_press(Message::UnregisterApplication))
                    .push(button("打开默认应用设置").on_press(Message::OpenDefaultAppsSettings)),
            )
            .into()
    }

    fn view_settings(&self, window_id: window::Id) -> Element<'_, Message> {
        let mut content = Column::new()
            .spacing(12)
            .push(text("窗口").size(24))
            .push(
                checkbox(self.config.bring_new_windows_to_front)
                    .label("打开新窗口时自动置顶")
                    .on_toggle(Message::BringNewWindowsToFrontChanged),
            )
            .push(rule::horizontal(1))
            .push(text("自定义应用").size(24));

        for (index, app) in self.config.custom_apps.iter().enumerate() {
            let app_editor = Column::new()
                .spacing(8)
                .push(
                    Row::new()
                        .spacing(8)
                        .push(text("名称").width(Length::Fixed(80.0)))
                        .push(
                            text_input("名称", &app.name)
                                .on_input(move |value| Message::CustomAppNameChanged(index, value))
                                .width(Length::Fill),
                        )
                        .push(button("移除").on_press(Message::RemoveCustomApp(index))),
                )
                .push(
                    Row::new()
                        .spacing(8)
                        .push(text("可执行文件").width(Length::Fixed(80.0)))
                        .push(
                            text_input("可执行文件", &app.executable)
                                .on_input(move |value| {
                                    Message::CustomAppExecutableChanged(index, value)
                                })
                                .width(Length::Fill),
                        ),
                )
                .push(
                    Row::new()
                        .spacing(8)
                        .push(text("参数").width(Length::Fixed(80.0)))
                        .push(
                            text_input("参数", &app.args_template)
                                .on_input(move |value| Message::CustomAppArgsChanged(index, value))
                                .width(Length::Fill),
                        ),
                );
            content = content.push(
                container(app_editor)
                    .padding(10)
                    .width(Length::Fill)
                    .style(iced::widget::container::rounded_box),
            );
        }

        content = content
            .push(text("添加自定义应用").size(18))
            .push(
                Row::new()
                    .spacing(8)
                    .push(text("名称").width(Length::Fixed(80.0)))
                    .push(
                        text_input("名称", &self.new_custom_app.name)
                            .on_input(Message::NewCustomAppNameChanged)
                            .width(Length::Fill),
                    ),
            )
            .push(
                Row::new()
                    .spacing(8)
                    .push(text("可执行文件").width(Length::Fixed(80.0)))
                    .push(
                        text_input("可执行文件", &self.new_custom_app.executable)
                            .on_input(Message::NewCustomAppExecutableChanged)
                            .width(Length::Fill),
                    ),
            )
            .push(
                Row::new()
                    .spacing(8)
                    .push(text("参数").width(Length::Fixed(80.0)))
                    .push(
                        text_input("参数", &self.new_custom_app.args_template)
                            .on_input(Message::NewCustomAppArgsChanged)
                            .width(Length::Fill),
                    )
                    .push(button("添加").on_press(Message::AddCustomApp)),
            )
            .push(rule::horizontal(1))
            .push(text("域名规则").size(24));

        for (index, rule) in self.config.domain_rules.iter().enumerate() {
            content = content.push(
                Row::new()
                    .spacing(8)
                    .push(text("匹配模式").width(Length::Fixed(80.0)))
                    .push(
                        text_input("匹配模式", &rule.pattern)
                            .on_input(move |value| Message::DomainRulePatternChanged(index, value))
                            .width(Length::Fill),
                    )
                    .push(text("应用").width(Length::Fixed(48.0)))
                    .push(
                        text_input("应用名称", &rule.app_name)
                            .on_input(move |value| Message::DomainRuleAppNameChanged(index, value))
                            .width(Length::Fill),
                    )
                    .push(button("移除").on_press(Message::RemoveDomainRule(index))),
            );
        }

        content = content
            .push(text("添加域名规则").size(18))
            .push(
                Row::new()
                    .spacing(8)
                    .push(text("匹配模式").width(Length::Fixed(80.0)))
                    .push(
                        text_input("匹配模式", &self.new_domain_rule.pattern)
                            .on_input(Message::NewDomainRulePatternChanged)
                            .width(Length::Fill),
                    ),
            )
            .push(
                Row::new()
                    .spacing(8)
                    .push(text("应用名称").width(Length::Fixed(80.0)))
                    .push(
                        text_input("应用名称", &self.new_domain_rule.app_name)
                            .on_input(Message::NewDomainRuleAppNameChanged)
                            .width(Length::Fill),
                    )
                    .push(button("添加").on_press(Message::AddDomainRule)),
            )
            .push(rule::horizontal(1))
            .push(
                Row::new()
                    .spacing(8)
                    .push(button("保存设置").on_press(Message::SaveSettings))
                    .push(button("恢复默认设置").on_press(Message::RequestResetConfig(window_id))),
            );

        scrollable(content).height(Length::Fill).into()
    }

    fn view_intercept<'a>(
        &'a self,
        window_id: window::Id,
        window: &'a InterceptWindow,
    ) -> Element<'a, Message> {
        let url = window.url();
        let favorite_label = if storage::is_favorite(&self.favorites, url.trim()) {
            "取消收藏"
        } else {
            "收藏"
        };
        let mut candidates_list = Column::new().spacing(8);
        for candidate in candidates::build_candidates(&self.config, url.trim()) {
            let open_button = button("打开").on_press_maybe(
                candidate
                    .available
                    .then_some(Message::OpenCandidate(window_id, candidate.clone())),
            );
            let row = Row::new()
                .spacing(12)
                .push(open_button)
                .push(text(candidate.name.clone()).width(Length::Fixed(180.0)))
                .push(text(candidate_kind_label(&candidate.kind)).width(Length::Fixed(120.0)))
                .push(text(candidate.reason));
            candidates_list = candidates_list.push(row).push(rule::horizontal(1));
        }

        let status = if window.status.is_empty() {
            "就绪".to_owned()
        } else {
            window.status.clone()
        };

        container(
            Column::new()
                .spacing(12)
                .padding(16)
                .push(text("拦截到的 URL").size(24))
                .push(
                    responsive(move |size| {
                        text_editor(&window.content)
                            .placeholder("URL 或 deeplink")
                            .on_action(move |action| Message::InterceptEdited(window_id, action))
                            .height(Length::Fixed(130.0))
                            .width(size.width.max(320.0))
                            .into()
                    })
                    .height(Length::Fixed(130.0)),
                )
                .push(
                    Row::new()
                        .spacing(8)
                        .push(button("复制").on_press(Message::CopyInterceptUrl(window_id)))
                        .push(
                            button(favorite_label)
                                .on_press(Message::ToggleInterceptFavorite(window_id)),
                        )
                        .push(
                            button("保存到历史记录")
                                .on_press(Message::SaveInterceptHistory(window_id)),
                        ),
                )
                .push(rule::horizontal(1))
                .push(text("打开方式").size(24))
                .push(scrollable(candidates_list).height(Length::Fill))
                .push(rule::horizontal(1))
                .push(text(status)),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

fn window_settings(mode: AppMode) -> window::Settings {
    window::Settings {
        size: mode.initial_size(),
        min_size: Some(mode.min_size()),
        exit_on_close_request: false,
        ..window::Settings::default()
    }
}

fn bring_window_to_front(window_id: window::Id) -> Task<Message> {
    Task::batch([
        window::minimize(window_id, false),
        window::gain_focus(window_id),
        window::request_user_attention(window_id, Some(window::UserAttention::Informational)),
    ])
}

fn runtime_event(
    event: Event,
    _status: event::Status,
    window_id: window::Id,
) -> Option<(window::Id, Event)> {
    match event {
        Event::Mouse(mouse::Event::CursorMoved { .. }) | Event::Keyboard(_) => {
            Some((window_id, event))
        }
        _ => None,
    }
}

fn is_w_key(key: &keyboard::Key) -> bool {
    matches!(key.as_ref(), keyboard::Key::Character("w" | "W"))
}

fn tab_button(active_tab: Tab, tab: Tab, label: &'static str) -> Element<'static, Message> {
    let label = if active_tab == tab {
        format!("> {label}")
    } else {
        label.to_owned()
    };
    button(text(label)).on_press(Message::SelectTab(tab)).into()
}

fn confirmation_overlay(
    target_pos: Point,
    message: &'static str,
    confirm_label: &'static str,
    cancel_message: Message,
    confirm_message: Message,
) -> Element<'static, Message> {
    let x = (target_pos.x - 60.0).max(0.0);
    let y = (target_pos.y - 60.0).max(0.0);
    let popup = container(
        Column::new().spacing(10).push(text(message)).push(
            Row::new()
                .spacing(8)
                .push(
                    button("取消")
                        .width(Length::Fixed(96.0))
                        .height(Length::Fixed(32.0))
                        .on_press(cancel_message),
                )
                .push(
                    button(confirm_label)
                        .width(Length::Fixed(96.0))
                        .height(Length::Fixed(32.0))
                        .on_press(confirm_message),
                ),
        ),
    )
    .padding(12)
    .width(Length::Fixed(420.0))
    .style(iced::widget::container::rounded_box);

    container(
        Column::new()
            .push(Space::new().height(Length::Fixed(y)))
            .push(
                Row::new()
                    .push(Space::new().width(Length::Fixed(x)))
                    .push(popup),
            ),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn candidate_kind_label(kind: &CandidateKind) -> &'static str {
    match kind {
        CandidateKind::Browser => "浏览器",
        CandidateKind::ProtocolHandler => "协议处理程序",
        CandidateKind::DomainApp => "域名应用",
        CandidateKind::CustomApp => "自定义应用",
        CandidateKind::ShellFallback => "Windows 默认处理程序",
    }
}
