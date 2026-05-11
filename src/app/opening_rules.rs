use super::{bring_window_to_front, LinkInterceptorApp};
use crate::{
    models::{Config, CustomApp, DomainRule, ProtocolRule},
    rules::{normalize_protocol_scheme, normalize_root_domain},
};
use floem::{
    action::focus_window,
    keyboard::{Key, NamedKey},
    peniko::kurbo::Size,
    peniko::Color,
    prelude::*,
    reactive::{RwSignal, SignalGet, SignalUpdate},
    views::{button, dropdown::Dropdown, dyn_stack, dyn_view, h_stack, text, text_input, v_stack},
    window::{close_window, new_window, WindowConfig, WindowId},
    IntoView,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpeningRuleEditor {
    AddCustomApp,
    EditCustomApp(usize),
    AddDomainRule,
    EditDomainRule(usize),
    AddProtocolRule,
    EditProtocolRule(usize),
}

impl OpeningRuleEditor {
    fn title(self) -> &'static str {
        match self {
            Self::AddCustomApp => "新增自定义打开目标",
            Self::EditCustomApp(_) => "编辑自定义打开目标",
            Self::AddDomainRule => "新增根域名规则",
            Self::EditDomainRule(_) => "编辑根域名规则",
            Self::AddProtocolRule => "新增 URI 协议规则",
            Self::EditProtocolRule(_) => "编辑 URI 协议规则",
        }
    }

    fn window_size(self) -> Size {
        match self {
            Self::AddCustomApp | Self::EditCustomApp(_) => Size::new(600.0, 250.0),
            Self::AddDomainRule
            | Self::EditDomainRule(_)
            | Self::AddProtocolRule
            | Self::EditProtocolRule(_) => Size::new(560.0, 220.0),
        }
    }

    fn minimum_size(self) -> Size {
        match self {
            Self::AddCustomApp | Self::EditCustomApp(_) => Size::new(520.0, 230.0),
            Self::AddDomainRule
            | Self::EditDomainRule(_)
            | Self::AddProtocolRule
            | Self::EditProtocolRule(_) => Size::new(480.0, 210.0),
        }
    }
}

#[derive(Clone)]
pub(super) struct OpeningRulesPanel {
    app: LinkInterceptorApp,
    error: RwSignal<String>,
    delete_custom_confirmation: RwSignal<Option<usize>>,
}

impl OpeningRulesPanel {
    pub(super) fn new(app: LinkInterceptorApp) -> Self {
        Self {
            app,
            error: RwSignal::new(String::new()),
            delete_custom_confirmation: RwSignal::new(None),
        }
    }

    pub(super) fn reset(&self) {
        self.delete_custom_confirmation.set(None);
        self.error.set(String::new());
    }

    pub(super) fn view(&self) -> impl IntoView + 'static {
        v_stack((
            text("自定义打开目标与规则").style(|s| s.font_size(22.0)),
            self.custom_apps_section(),
            self.domain_rules_section(),
            self.protocol_rules_section(),
            self.error_view(),
        ))
        .style(|s| s.width_full().gap(14).flex_col())
    }

    fn custom_apps_section(&self) -> impl IntoView + 'static {
        v_stack((
            h_stack((
                v_stack((
                    text("自定义打开目标").style(|s| s.font_size(18.0)),
                    text("维护可执行文件和参数模板。目标只会在根域名规则或 URI 协议规则命中时出现在打开方式中。")
                        .style(|s| s.font_size(12.0).color(Color::rgb8(100, 100, 100))),
                ))
                .style(|s| s.flex_col().gap(2).flex_grow(1.0).min_width(0.0)),
                button("新增").action({
                    let panel = self.clone();
                    move || panel.begin_add_custom_app()
                }),
            ))
            .style(|s| s.gap(8).items_start().width_full()),
            dyn_view({
                let panel = self.clone();
                move || {
                    if panel.app.state.config.get().custom_apps.is_empty() {
                        empty_text("尚未添加自定义打开目标。").into_any()
                    } else {
                        panel.custom_apps_list().into_any()
                    }
                }
            }),
            self.delete_custom_confirmation_view(),
        ))
        .style(|s| section_style(s))
    }

    fn domain_rules_section(&self) -> impl IntoView + 'static {
        let panel = self.clone();
        v_stack((
            h_stack((
                v_stack((
                    text("根域名规则").style(|s| s.font_size(18.0)),
                    text("仅比较根域名；例如 api.example.com 会按 example.com 保存和匹配。")
                        .style(|s| s.font_size(12.0).color(Color::rgb8(100, 100, 100))),
                ))
                .style(|s| s.flex_col().gap(2).flex_grow(1.0).min_width(0.0)),
                button("新增")
                    .action({
                        let panel = self.clone();
                        move || panel.begin_add_domain_rule()
                    })
                    .disabled({
                        let panel = self.clone();
                        move || panel.app.state.config.get().custom_apps.is_empty()
                    }),
            ))
            .style(|s| s.gap(8).items_start().width_full()),
            dyn_view({
                let panel = panel.clone();
                move || {
                    let config = panel.app.state.config.get();
                    if config.custom_apps.is_empty() {
                        empty_text("请先新增自定义打开目标，再配置根域名规则。").into_any()
                    } else if config.domain_rules.is_empty() {
                        empty_text("尚未添加根域名规则。").into_any()
                    } else {
                        panel.domain_rules_list().into_any()
                    }
                }
            }),
        ))
        .style(|s| section_style(s))
    }

    fn protocol_rules_section(&self) -> impl IntoView + 'static {
        let panel = self.clone();
        v_stack((
            h_stack((
                v_stack((
                    text("URI 协议规则").style(|s| s.font_size(18.0)),
                    text("配置 mailto、slack、web+demo 等 URI 协议命中后可用的自定义打开目标。")
                        .style(|s| s.font_size(12.0).color(Color::rgb8(100, 100, 100))),
                ))
                .style(|s| s.flex_col().gap(2).flex_grow(1.0).min_width(0.0)),
                button("新增")
                    .action({
                        let panel = self.clone();
                        move || panel.begin_add_protocol_rule()
                    })
                    .disabled({
                        let panel = self.clone();
                        move || panel.app.state.config.get().custom_apps.is_empty()
                    }),
            ))
            .style(|s| s.gap(8).items_start().width_full()),
            dyn_view({
                let panel = panel.clone();
                move || {
                    let config = panel.app.state.config.get();
                    if config.custom_apps.is_empty() {
                        empty_text("请先新增自定义打开目标，再配置 URI 协议规则。").into_any()
                    } else if config.protocol_rules.is_empty() {
                        empty_text("尚未添加 URI 协议规则。").into_any()
                    } else {
                        panel.protocol_rules_list().into_any()
                    }
                }
            }),
        ))
        .style(|s| section_style(s))
    }

    fn custom_apps_list(&self) -> impl IntoView + 'static {
        let panel = self.clone();
        dyn_stack(
            move || {
                panel
                    .app
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
                let panel = self.clone();
                move |(index, app): (usize, CustomApp)| panel.custom_app_row(index, app)
            },
        )
        .style(|s| s.flex_col().gap(6).width_full())
    }

    fn domain_rules_list(&self) -> impl IntoView + 'static {
        let panel = self.clone();
        dyn_stack(
            move || {
                panel
                    .app
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
                let panel = self.clone();
                move |(index, rule): (usize, DomainRule)| panel.domain_rule_row(index, rule)
            },
        )
        .style(|s| s.flex_col().gap(6).width_full())
    }

    fn protocol_rules_list(&self) -> impl IntoView + 'static {
        let panel = self.clone();
        dyn_stack(
            move || {
                panel
                    .app
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
                let panel = self.clone();
                move |(index, rule): (usize, ProtocolRule)| panel.protocol_rule_row(index, rule)
            },
        )
        .style(|s| s.flex_col().gap(6).width_full())
    }

    fn custom_app_row(&self, index: usize, app: CustomApp) -> floem::AnyView {
        h_stack((
            v_stack((
                text(app.name).style(|s| s.font_size(15.0)),
                text(format!("可执行文件：{}", display_or_empty(&app.executable)))
                    .style(meta_style),
                text(format!("参数：{}", display_or_empty(&app.args_template))).style(meta_style),
            ))
            .style(|s| s.flex_col().gap(2).flex_grow(1.0).min_width(0.0)),
            button("编辑").action({
                let panel = self.clone();
                move || panel.begin_edit_custom_app(index)
            }),
            button("删除").action({
                let panel = self.clone();
                move || panel.request_delete_custom_app(index)
            }),
        ))
        .style(|s| row_style(s))
        .into_any()
    }

    fn domain_rule_row(&self, index: usize, rule: DomainRule) -> floem::AnyView {
        h_stack((
            v_stack((
                text(rule.pattern).style(|s| s.font_size(15.0)),
                text(format!("自定义应用：{}", display_or_empty(&rule.app_name))).style(meta_style),
            ))
            .style(|s| s.flex_col().gap(2).flex_grow(1.0).min_width(0.0)),
            button("编辑").action({
                let panel = self.clone();
                move || panel.begin_edit_domain_rule(index)
            }),
            button("删除").action({
                let panel = self.clone();
                move || panel.delete_domain_rule(index)
            }),
        ))
        .style(|s| row_style(s))
        .into_any()
    }

    fn protocol_rule_row(&self, index: usize, rule: ProtocolRule) -> floem::AnyView {
        h_stack((
            v_stack((
                text(rule.scheme).style(|s| s.font_size(15.0)),
                text(format!("自定义应用：{}", display_or_empty(&rule.app_name))).style(meta_style),
            ))
            .style(|s| s.flex_col().gap(2).flex_grow(1.0).min_width(0.0)),
            button("编辑").action({
                let panel = self.clone();
                move || panel.begin_edit_protocol_rule(index)
            }),
            button("删除").action({
                let panel = self.clone();
                move || panel.delete_protocol_rule(index)
            }),
        ))
        .style(|s| row_style(s))
        .into_any()
    }

    fn error_view(&self) -> impl IntoView + 'static {
        let panel = self.clone();
        dyn_view(move || {
            let error = panel.error.get();
            if error.is_empty() {
                text("").into_any()
            } else {
                text(error)
                    .style(|s| s.color(Color::rgb8(180, 40, 40)).font_size(12.0))
                    .into_any()
            }
        })
    }

    fn delete_custom_confirmation_view(&self) -> impl IntoView + 'static {
        let panel = self.clone();
        dyn_view(move || {
            let Some(index) = panel.delete_custom_confirmation.get() else {
                return text("").into_any();
            };
            let config = panel.app.state.config.get();
            let Some(app) = config.custom_apps.get(index) else {
                return text("").into_any();
            };
            let reference_count = custom_app_reference_count(&config, &app.name);
            h_stack((
                text(format!(
                    "“{}”被 {} 条规则引用。确认删除会同时删除这些规则。",
                    app.name, reference_count
                )),
                button("取消").action({
                    let panel = panel.clone();
                    move || panel.delete_custom_confirmation.set(None)
                }),
                button("确认删除").action({
                    let panel = panel.clone();
                    move || panel.confirm_delete_custom_app(index)
                }),
            ))
            .style(|s| s.gap(8).items_center().padding(8))
            .into_any()
        })
    }

    fn begin_add_custom_app(&self) {
        self.error.set(String::new());
        self.delete_custom_confirmation.set(None);
        self.open_editor_window(OpeningRuleEditor::AddCustomApp);
    }

    fn begin_edit_custom_app(&self, index: usize) {
        let config = self.app.state.config.get();
        if config.custom_apps.get(index).is_none() {
            self.set_error("要编辑的自定义打开目标不存在");
            return;
        }

        self.error.set(String::new());
        self.delete_custom_confirmation.set(None);
        self.open_editor_window(OpeningRuleEditor::EditCustomApp(index));
    }

    fn begin_add_domain_rule(&self) {
        if self.first_app_name().is_none() {
            self.set_error("请先新增自定义打开目标");
            return;
        }

        self.error.set(String::new());
        self.delete_custom_confirmation.set(None);
        self.open_editor_window(OpeningRuleEditor::AddDomainRule);
    }

    fn begin_edit_domain_rule(&self, index: usize) {
        let config = self.app.state.config.get();
        if config.domain_rules.get(index).is_none() {
            self.set_error("要编辑的根域名规则不存在");
            return;
        }

        self.error.set(String::new());
        self.delete_custom_confirmation.set(None);
        self.open_editor_window(OpeningRuleEditor::EditDomainRule(index));
    }

    fn begin_add_protocol_rule(&self) {
        if self.first_app_name().is_none() {
            self.set_error("请先新增自定义打开目标");
            return;
        }

        self.error.set(String::new());
        self.delete_custom_confirmation.set(None);
        self.open_editor_window(OpeningRuleEditor::AddProtocolRule);
    }

    fn begin_edit_protocol_rule(&self, index: usize) {
        let config = self.app.state.config.get();
        if config.protocol_rules.get(index).is_none() {
            self.set_error("要编辑的 URI 协议规则不存在");
            return;
        }

        self.error.set(String::new());
        self.delete_custom_confirmation.set(None);
        self.open_editor_window(OpeningRuleEditor::EditProtocolRule(index));
    }

    fn open_editor_window(&self, editor: OpeningRuleEditor) {
        let app = self.app.clone();
        new_window(
            move |window_id| {
                if app.state.config.get().bring_new_windows_to_front {
                    bring_window_to_front(window_id);
                    focus_window();
                }
                OpeningRuleEditorWindow::new(app.clone(), window_id, editor).view()
            },
            Some(editor_window_config(editor)),
        );
    }

    fn request_delete_custom_app(&self, index: usize) {
        self.error.set(String::new());
        let config = self.app.state.config.get();
        let Some(app) = config.custom_apps.get(index) else {
            self.set_error("要删除的自定义打开目标不存在");
            return;
        };

        if custom_app_reference_count(&config, &app.name) > 0 {
            self.delete_custom_confirmation.set(Some(index));
        } else {
            self.confirm_delete_custom_app(index);
        }
    }

    fn confirm_delete_custom_app(&self, index: usize) {
        let mut deleted = false;
        self.app.state.config.update(|config| {
            if index < config.custom_apps.len() {
                let name = config.custom_apps.remove(index).name;
                config
                    .domain_rules
                    .retain(|rule| !rule.app_name.eq_ignore_ascii_case(&name));
                config
                    .protocol_rules
                    .retain(|rule| !rule.app_name.eq_ignore_ascii_case(&name));
                deleted = true;
            }
        });

        self.delete_custom_confirmation.set(None);
        if deleted {
            self.app.persist_config();
            self.app.state.status.set("自定义打开目标已删除".to_owned());
        } else {
            self.set_error("要删除的自定义打开目标不存在");
        }
    }

    fn delete_domain_rule(&self, index: usize) {
        let mut deleted = false;
        self.app.state.config.update(|config| {
            if index < config.domain_rules.len() {
                config.domain_rules.remove(index);
                deleted = true;
            }
        });
        if deleted {
            self.app.persist_config();
            self.app.state.status.set("根域名规则已删除".to_owned());
        } else {
            self.set_error("要删除的根域名规则不存在");
        }
    }

    fn delete_protocol_rule(&self, index: usize) {
        let mut deleted = false;
        self.app.state.config.update(|config| {
            if index < config.protocol_rules.len() {
                config.protocol_rules.remove(index);
                deleted = true;
            }
        });
        if deleted {
            self.app.persist_config();
            self.app.state.status.set("URI 协议规则已删除".to_owned());
        } else {
            self.set_error("要删除的 URI 协议规则不存在");
        }
    }

    fn first_app_name(&self) -> Option<String> {
        self.app
            .state
            .config
            .get()
            .custom_apps
            .first()
            .map(|app| app.name.clone())
    }

    fn set_error(&self, message: impl Into<String>) {
        let message = message.into();
        self.error.set(message.clone());
        self.app.state.status.set(message);
    }
}

#[derive(Clone)]
struct OpeningRuleEditorWindow {
    app: LinkInterceptorApp,
    window_id: WindowId,
    editor: OpeningRuleEditor,
    custom_name: RwSignal<String>,
    custom_executable: RwSignal<String>,
    custom_args: RwSignal<String>,
    domain_pattern: RwSignal<String>,
    domain_app_name: RwSignal<String>,
    protocol_scheme: RwSignal<String>,
    protocol_app_name: RwSignal<String>,
    error: RwSignal<String>,
}

impl OpeningRuleEditorWindow {
    fn new(app: LinkInterceptorApp, window_id: WindowId, editor: OpeningRuleEditor) -> Self {
        crate::native_window::set_minimum_content_size(window_id, editor.minimum_size());
        let config = app.state.config.get();
        let mut custom_name = String::new();
        let mut custom_executable = String::new();
        let mut custom_args = "{url}".to_owned();
        let mut domain_pattern = String::new();
        let mut domain_app_name = config
            .custom_apps
            .first()
            .map(|app| app.name.clone())
            .unwrap_or_default();
        let mut protocol_scheme = String::new();
        let mut protocol_app_name = domain_app_name.clone();

        match editor {
            OpeningRuleEditor::EditCustomApp(index) => {
                if let Some(app) = config.custom_apps.get(index) {
                    custom_name = app.name.clone();
                    custom_executable = app.executable.clone();
                    custom_args = app.args_template.clone();
                }
            }
            OpeningRuleEditor::EditDomainRule(index) => {
                if let Some(rule) = config.domain_rules.get(index) {
                    domain_pattern = rule.pattern.clone();
                    domain_app_name = rule.app_name.clone();
                }
            }
            OpeningRuleEditor::EditProtocolRule(index) => {
                if let Some(rule) = config.protocol_rules.get(index) {
                    protocol_scheme = rule.scheme.clone();
                    protocol_app_name = rule.app_name.clone();
                }
            }
            OpeningRuleEditor::AddCustomApp
            | OpeningRuleEditor::AddDomainRule
            | OpeningRuleEditor::AddProtocolRule => {}
        }

        Self {
            app,
            window_id,
            editor,
            custom_name: RwSignal::new(custom_name),
            custom_executable: RwSignal::new(custom_executable),
            custom_args: RwSignal::new(custom_args),
            domain_pattern: RwSignal::new(domain_pattern),
            domain_app_name: RwSignal::new(domain_app_name),
            protocol_scheme: RwSignal::new(protocol_scheme),
            protocol_app_name: RwSignal::new(protocol_app_name),
            error: RwSignal::new(String::new()),
        }
    }

    fn view(self) -> floem::AnyView {
        let window_id = self.window_id;
        let content = match self.editor {
            OpeningRuleEditor::AddCustomApp => self.custom_app_editor(None).into_any(),
            OpeningRuleEditor::EditCustomApp(index) => {
                self.custom_app_editor(Some(index)).into_any()
            }
            OpeningRuleEditor::AddDomainRule => self.domain_rule_editor(None).into_any(),
            OpeningRuleEditor::EditDomainRule(index) => {
                self.domain_rule_editor(Some(index)).into_any()
            }
            OpeningRuleEditor::AddProtocolRule => self.protocol_rule_editor(None).into_any(),
            OpeningRuleEditor::EditProtocolRule(index) => {
                self.protocol_rule_editor(Some(index)).into_any()
            }
        };

        content
            .on_key_down(
                Key::Character("w".into()),
                |modifiers| modifiers.control(),
                move |_| {
                    close_window(window_id);
                },
            )
            .on_key_down(
                Key::Named(NamedKey::Escape),
                |_| true,
                move |_| {
                    close_window(window_id);
                },
            )
            .style(|s| s.size_full().padding(16))
            .into_any()
    }

    fn custom_app_editor(&self, index: Option<usize>) -> impl IntoView + 'static {
        v_stack((
            text(if index.is_some() {
                "编辑自定义打开目标"
            } else {
                "新增自定义打开目标"
            })
            .style(|s| s.font_size(18.0)),
            form_row("名称", text_input(self.custom_name).style(input_style)),
            form_row(
                "可执行文件",
                text_input(self.custom_executable).style(input_style),
            ),
            form_row("参数模板", text_input(self.custom_args).style(input_style)),
            self.error_view(),
            h_stack((
                button("取消").action({
                    let editor = self.clone();
                    move || close_window(editor.window_id)
                }),
                button("保存").action({
                    let editor = self.clone();
                    move || editor.save_custom_app(index)
                }),
            ))
            .style(|s| s.gap(8)),
        ))
        .style(editor_window_style)
    }

    fn domain_rule_editor(&self, index: Option<usize>) -> impl IntoView + 'static {
        v_stack((
            text(if index.is_some() {
                "编辑根域名规则"
            } else {
                "新增根域名规则"
            })
            .style(|s| s.font_size(18.0)),
            form_row("根域名", text_input(self.domain_pattern).style(input_style)),
            self.app_dropdown_row("自定义应用", self.domain_app_name),
            self.error_view(),
            h_stack((
                button("取消").action({
                    let editor = self.clone();
                    move || close_window(editor.window_id)
                }),
                button("保存").action({
                    let editor = self.clone();
                    move || editor.save_domain_rule(index)
                }),
            ))
            .style(|s| s.gap(8)),
        ))
        .style(editor_window_style)
    }

    fn protocol_rule_editor(&self, index: Option<usize>) -> impl IntoView + 'static {
        v_stack((
            text(if index.is_some() {
                "编辑 URI 协议规则"
            } else {
                "新增 URI 协议规则"
            })
            .style(|s| s.font_size(18.0)),
            form_row("协议", text_input(self.protocol_scheme).style(input_style)),
            self.app_dropdown_row("自定义应用", self.protocol_app_name),
            self.error_view(),
            h_stack((
                button("取消").action({
                    let editor = self.clone();
                    move || close_window(editor.window_id)
                }),
                button("保存").action({
                    let editor = self.clone();
                    move || editor.save_protocol_rule(index)
                }),
            ))
            .style(|s| s.gap(8)),
        ))
        .style(editor_window_style)
    }

    fn app_dropdown_row(
        &self,
        label_text: &'static str,
        selected: RwSignal<String>,
    ) -> impl IntoView + 'static {
        h_stack((
            text(label_text).style(|s| s.width(90.0).flex_shrink(0.0)),
            dyn_view({
                let editor = self.clone();
                move || {
                    let app_names = editor.app_names();
                    if app_names.is_empty() {
                        text("请先新增自定义打开目标").into_any()
                    } else {
                        Dropdown::new_rw(selected, app_names)
                            .style(|s| s.width(260.0).min_width(160.0))
                            .into_any()
                    }
                }
            }),
        ))
        .style(|s| s.gap(8).items_center().width_full())
    }

    fn error_view(&self) -> impl IntoView + 'static {
        let editor = self.clone();
        dyn_view(move || {
            let error = editor.error.get();
            if error.is_empty() {
                text("").into_any()
            } else {
                text(error)
                    .style(|s| s.color(Color::rgb8(180, 40, 40)).font_size(12.0))
                    .into_any()
            }
        })
    }

    fn save_custom_app(&self, index: Option<usize>) {
        let name = self.custom_name.get().trim().to_owned();
        let executable = self.custom_executable.get().trim().to_owned();
        let args_template = self.custom_args.get().trim().to_owned();
        let config = self.app.state.config.get();

        if name.is_empty() {
            self.set_error("自定义打开目标名称不能为空");
            return;
        }
        if executable.is_empty() {
            self.set_error("可执行文件不能为空");
            return;
        }
        if custom_app_name_exists(&config, &name, index) {
            self.set_error("自定义打开目标名称不能重复");
            return;
        }

        let mut updated = false;
        self.app.state.config.update(|config| match index {
            Some(index) => {
                if index < config.custom_apps.len() {
                    let old_name = config.custom_apps[index].name.clone();
                    config.custom_apps[index] = CustomApp {
                        name: name.clone(),
                        executable: executable.clone(),
                        args_template: args_template.clone(),
                    };
                    rename_custom_app_references(config, &old_name, &name);
                    updated = true;
                }
            }
            None => {
                config.custom_apps.push(CustomApp {
                    name: name.clone(),
                    executable: executable.clone(),
                    args_template: args_template.clone(),
                });
                updated = true;
            }
        });

        if !updated {
            self.set_error("要保存的自定义打开目标不存在");
            return;
        }
        self.finish_save("自定义打开目标已保存");
    }

    fn save_domain_rule(&self, index: Option<usize>) {
        let Some(pattern) = normalize_root_domain(&self.domain_pattern.get()) else {
            self.set_error("根域名无效");
            return;
        };
        let app_name = self.domain_app_name.get().trim().to_owned();
        let config = self.app.state.config.get();

        if !custom_app_exists(&config, &app_name) {
            self.set_error("请选择有效的自定义打开目标");
            return;
        }
        if domain_rule_exists(&config, &pattern, &app_name, index) {
            self.set_error("相同根域名和自定义打开目标的规则已存在");
            return;
        }

        let mut updated = false;
        self.app.state.config.update(|config| match index {
            Some(index) => {
                if let Some(rule) = config.domain_rules.get_mut(index) {
                    rule.pattern = pattern.clone();
                    rule.app_name = app_name.clone();
                    updated = true;
                }
            }
            None => {
                config.domain_rules.push(DomainRule {
                    pattern: pattern.clone(),
                    app_name: app_name.clone(),
                });
                updated = true;
            }
        });

        if !updated {
            self.set_error("要保存的根域名规则不存在");
            return;
        }
        self.finish_save("根域名规则已保存");
    }

    fn save_protocol_rule(&self, index: Option<usize>) {
        let Some(scheme) = normalize_protocol_scheme(&self.protocol_scheme.get()) else {
            self.set_error("URI 协议无效");
            return;
        };
        let app_name = self.protocol_app_name.get().trim().to_owned();
        let config = self.app.state.config.get();

        if !custom_app_exists(&config, &app_name) {
            self.set_error("请选择有效的自定义打开目标");
            return;
        }
        if protocol_rule_exists(&config, &scheme, &app_name, index) {
            self.set_error("相同 URI 协议和自定义打开目标的规则已存在");
            return;
        }

        let mut updated = false;
        self.app.state.config.update(|config| match index {
            Some(index) => {
                if let Some(rule) = config.protocol_rules.get_mut(index) {
                    rule.scheme = scheme.clone();
                    rule.app_name = app_name.clone();
                    updated = true;
                }
            }
            None => {
                config.protocol_rules.push(ProtocolRule {
                    scheme: scheme.clone(),
                    app_name: app_name.clone(),
                });
                updated = true;
            }
        });

        if !updated {
            self.set_error("要保存的 URI 协议规则不存在");
            return;
        }
        self.finish_save("URI 协议规则已保存");
    }

    fn app_names(&self) -> Vec<String> {
        self.app
            .state
            .config
            .get()
            .custom_apps
            .into_iter()
            .map(|app| app.name)
            .collect()
    }

    fn finish_save(&self, status: &str) {
        self.error.set(String::new());
        self.app.persist_config();
        self.app.state.status.set(status.to_owned());
        close_window(self.window_id);
    }

    fn set_error(&self, message: impl Into<String>) {
        let message = message.into();
        self.error.set(message.clone());
        self.app.state.status.set(message);
    }
}

fn custom_app_exists(config: &Config, name: &str) -> bool {
    config
        .custom_apps
        .iter()
        .any(|app| app.name.eq_ignore_ascii_case(name.trim()))
}

fn custom_app_name_exists(config: &Config, name: &str, except_index: Option<usize>) -> bool {
    let name = name.trim();
    config
        .custom_apps
        .iter()
        .enumerate()
        .any(|(index, app)| Some(index) != except_index && app.name.eq_ignore_ascii_case(name))
}

fn domain_rule_exists(
    config: &Config,
    pattern: &str,
    app_name: &str,
    except_index: Option<usize>,
) -> bool {
    let Some(pattern) = normalize_root_domain(pattern) else {
        return false;
    };
    config.domain_rules.iter().enumerate().any(|(index, rule)| {
        Some(index) != except_index
            && rule.app_name.eq_ignore_ascii_case(app_name)
            && normalize_root_domain(&rule.pattern)
                .is_some_and(|existing| existing.eq_ignore_ascii_case(&pattern))
    })
}

fn protocol_rule_exists(
    config: &Config,
    scheme: &str,
    app_name: &str,
    except_index: Option<usize>,
) -> bool {
    let Some(scheme) = normalize_protocol_scheme(scheme) else {
        return false;
    };
    config
        .protocol_rules
        .iter()
        .enumerate()
        .any(|(index, rule)| {
            Some(index) != except_index
                && rule.app_name.eq_ignore_ascii_case(app_name)
                && normalize_protocol_scheme(&rule.scheme)
                    .is_some_and(|existing| existing.eq_ignore_ascii_case(&scheme))
        })
}

fn custom_app_reference_count(config: &Config, name: &str) -> usize {
    config
        .domain_rules
        .iter()
        .filter(|rule| rule.app_name.eq_ignore_ascii_case(name))
        .count()
        + config
            .protocol_rules
            .iter()
            .filter(|rule| rule.app_name.eq_ignore_ascii_case(name))
            .count()
}

fn rename_custom_app_references(config: &mut Config, old_name: &str, new_name: &str) {
    for rule in &mut config.domain_rules {
        if rule.app_name.eq_ignore_ascii_case(old_name) {
            rule.app_name = new_name.to_owned();
        }
    }
    for rule in &mut config.protocol_rules {
        if rule.app_name.eq_ignore_ascii_case(old_name) {
            rule.app_name = new_name.to_owned();
        }
    }
}

fn display_or_empty(value: &str) -> String {
    if value.trim().is_empty() {
        "未设置".to_owned()
    } else {
        value.to_owned()
    }
}

fn empty_text(message: &'static str) -> impl IntoView {
    text(message).style(|s| s.font_size(12.0).color(Color::rgb8(100, 100, 100)))
}

fn form_row(label_text: &'static str, input: impl IntoView + 'static) -> impl IntoView {
    h_stack((
        text(label_text).style(|s| s.width(90.0).flex_shrink(0.0)),
        input,
    ))
    .style(|s| s.gap(8).items_center().width_full())
}

fn meta_style(style: floem::style::Style) -> floem::style::Style {
    style
        .font_size(12.0)
        .color(Color::rgb8(100, 100, 100))
        .width_full()
        .min_width(0.0)
}

fn input_style(style: floem::style::Style) -> floem::style::Style {
    style.width(360.0).min_width(160.0)
}

fn section_style(style: floem::style::Style) -> floem::style::Style {
    style
        .width_full()
        .gap(8)
        .padding(12)
        .flex_col()
        .border(1.0)
        .border_color(Color::rgb8(226, 226, 226))
        .border_radius(8.0)
        .background(Color::rgb8(250, 250, 250))
}

fn row_style(style: floem::style::Style) -> floem::style::Style {
    style
        .gap(8)
        .items_start()
        .padding(8)
        .width_full()
        .min_width(0.0)
        .border(1.0)
        .border_color(Color::rgb8(224, 224, 224))
        .border_radius(6.0)
        .background(Color::rgb8(255, 255, 255))
}

fn editor_window_config(editor: OpeningRuleEditor) -> WindowConfig {
    WindowConfig::default()
        .title(editor.title())
        .size(editor.window_size())
}

fn editor_window_style(style: floem::style::Style) -> floem::style::Style {
    style
        .size_full()
        .gap(8)
        .flex_col()
        .min_width(0.0)
        .min_height(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        Config {
            custom_apps: vec![
                CustomApp {
                    name: "Example App".to_owned(),
                    executable: "example.exe".to_owned(),
                    args_template: "{url}".to_owned(),
                },
                CustomApp {
                    name: "Mail App".to_owned(),
                    executable: "mail.exe".to_owned(),
                    args_template: "{url}".to_owned(),
                },
            ],
            domain_rules: vec![DomainRule {
                pattern: "example.com".to_owned(),
                app_name: "Example App".to_owned(),
            }],
            protocol_rules: vec![ProtocolRule {
                scheme: "mailto".to_owned(),
                app_name: "Mail App".to_owned(),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn detects_duplicate_custom_app_names_case_insensitively() {
        let config = test_config();

        assert!(custom_app_name_exists(&config, "example app", None));
        assert!(!custom_app_name_exists(&config, "example app", Some(0)));
        assert!(!custom_app_name_exists(&config, "Other App", None));
    }

    #[test]
    fn detects_duplicate_domain_rules_after_normalization() {
        let config = test_config();

        assert!(domain_rule_exists(
            &config,
            "example.com",
            "Example App",
            None
        ));
        assert!(domain_rule_exists(
            &config,
            "www.example.com",
            "Example App",
            None
        ));
        assert!(!domain_rule_exists(
            &config,
            "example.com",
            "Mail App",
            None
        ));
    }

    #[test]
    fn detects_duplicate_protocol_rules_after_normalization() {
        let config = test_config();

        assert!(protocol_rule_exists(&config, "MAILTO", "Mail App", None));
        assert!(!protocol_rule_exists(
            &config,
            "mailto",
            "Example App",
            None
        ));
    }

    #[test]
    fn renames_custom_app_references() {
        let mut config = test_config();

        rename_custom_app_references(&mut config, "Example App", "Browser Helper");
        rename_custom_app_references(&mut config, "Mail App", "Mail Helper");

        assert_eq!(config.domain_rules[0].app_name, "Browser Helper");
        assert_eq!(config.protocol_rules[0].app_name, "Mail Helper");
    }
}
