use super::{AppMode, LinkInterceptorApp, MainTab};
use crate::{
    candidates,
    models::{CandidateKind, OpenCandidate},
    storage,
    ui_style::interactive_cursor_style,
};
use floem::{
    Clipboard, IntoView,
    keyboard::{Key, NamedKey},
    prelude::*,
    reactive::{RwSignal, SignalGet, SignalUpdate},
    taffy::style::FlexWrap,
    views::{
        button, dyn_stack, h_stack, label, labeled_checkbox, scroll, text, text_input, v_stack,
    },
    window::{WindowId, close_window},
};

#[derive(Clone)]
pub(super) struct InterceptWindow {
    app: LinkInterceptorApp,
    window_id: WindowId,
    url: RwSignal<String>,
    window_status: RwSignal<String>,
    close_after_open: RwSignal<bool>,
}

impl InterceptWindow {
    pub(super) fn new(app: LinkInterceptorApp, window_id: WindowId, initial_url: String) -> Self {
        crate::native_window::set_minimum_content_size(
            window_id,
            AppMode::InterceptWindow.minimum_size(),
        );
        let close_after_open =
            RwSignal::new(app.state.config.get().close_intercept_window_after_open);
        Self {
            app,
            window_id,
            url: RwSignal::new(initial_url),
            window_status: RwSignal::new(String::new()),
            close_after_open,
        }
    }

    pub(super) fn view(self) -> floem::AnyView {
        let window_id = self.window_id;
        let url = self.url;
        let window_status = self.window_status;
        let close_after_open = self.close_after_open;
        let app = self.app.clone();
        let candidates_app = self.app.clone();
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
                    if storage::is_favorite(&app.state.favorites.get(), url.get().trim()) {
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
                labeled_checkbox(
                    move || close_after_open.get(),
                    || "打开链接后自动关闭此窗口",
                )
                .on_update(move |checked| close_after_open.set(checked)),
            ))
            .style(|s| s.gap(8).items_center()),
            text("打开方式").style(|s| s.font_size(20.0)),
            scroll(
                dyn_stack(
                    move || {
                        candidates::build_candidates(&candidates_app.state.config.get(), &url.get())
                    },
                    |candidate| {
                        (
                            candidate.name.clone(),
                            format!("{:?}", candidate.kind),
                            candidate.command.clone(),
                            candidate.reason.clone(),
                        )
                    },
                    {
                        let app = app.clone();
                        move |candidate: OpenCandidate| {
                            let label = candidate_button_label(&candidate);
                            let enabled = candidate.available;
                            button(label)
                                .disabled(move || !enabled)
                                .action({
                                    let app = app.clone();
                                    let candidate = candidate.clone();
                                    let window_status = window_status;
                                    move || {
                                        let (status, opened) =
                                            app.open_candidate_for_url(&candidate, &url.get());
                                        window_status.set(status);
                                        if opened && close_after_open.get() {
                                            close_window(window_id);
                                        }
                                    }
                                })
                                .style(|s| s.flex_shrink(0.0))
                        }
                    },
                )
                .style(|s| {
                    s.flex_row()
                        .flex_wrap(FlexWrap::Wrap)
                        .gap(6)
                        .width_full()
                        .min_width(0.0)
                }),
            )
            .style(|s| s.flex_grow(1.0).min_height(64.0).width_full()),
            text("主界面").style(|s| s.font_size(20.0)),
            h_stack((
                main_tab_button("收藏", app.clone(), MainTab::Favorites),
                main_tab_button("历史记录", app.clone(), MainTab::History),
                main_tab_button("注册状态", app.clone(), MainTab::Registration),
                main_tab_button("设置", app.clone(), MainTab::Settings),
            ))
            .style(|s| s.gap(8).items_center()),
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
        .on_key_down(Key::Named(NamedKey::Escape), |_| true, move |_| {
            close_window(window_id);
        })
        .style(|s| interactive_cursor_style(s.size_full().padding(14).gap(10).flex_col()))
        .into_any()
    }
}

fn main_tab_button(
    label_text: &'static str,
    app: LinkInterceptorApp,
    tab: MainTab,
) -> impl IntoView {
    button(label_text).action(move || app.show_or_create_main_window_tab(tab))
}

fn candidate_button_label(candidate: &OpenCandidate) -> String {
    let kind = candidate_kind_label(candidate.kind.clone());
    let prefix = candidate_button_prefix(candidate, kind);
    let name = candidate.name.trim();

    if name.is_empty() || name == prefix {
        prefix
    } else {
        format!("{prefix}：{name}")
    }
}

fn candidate_button_prefix(candidate: &OpenCandidate, kind: &str) -> String {
    match candidate.kind.clone() {
        CandidateKind::Browser | CandidateKind::CustomApp | CandidateKind::ShellFallback => {
            kind.to_owned()
        }
        CandidateKind::ProtocolHandler => candidate
            .reason
            .trim()
            .strip_prefix("已注册的 ")
            .and_then(|reason| reason.strip_suffix(" 协议处理程序"))
            .map(|scheme| format!("{kind} · {scheme}"))
            .unwrap_or_else(|| kind.to_owned()),
        CandidateKind::DomainApp => {
            let reason = candidate.reason.trim();
            if let Some(pattern) = reason.strip_prefix("域名规则 ") {
                format!("{kind} · {pattern}")
            } else if let Some(domain) = reason.strip_prefix("Windows Apps for Websites 匹配 ") {
                format!("{kind} · Apps for Websites · {domain}")
            } else {
                kind.to_owned()
            }
        }
    }
}

fn candidate_kind_label(kind: CandidateKind) -> &'static str {
    match kind {
        CandidateKind::Browser => "浏览器",
        CandidateKind::ProtocolHandler => "协议处理程序",
        CandidateKind::DomainApp => "域名应用",
        CandidateKind::CustomApp => "自定义应用",
        CandidateKind::ShellFallback => "Windows 默认处理程序",
    }
}
