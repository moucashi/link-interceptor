use floem::{
    style::{CursorStyle, Style},
    style_class,
    views::{
        ButtonClass, CheckboxClass, LabeledCheckboxClass, LabeledRadioButtonClass, ListItemClass,
        RadioButtonClass, ToggleButtonClass, dropdown, slider,
    },
};

style_class!(pub(crate) ClickableClass);

pub(crate) fn interactive_cursor_style(style: Style) -> Style {
    style
        .class(ButtonClass, clickable_cursor)
        .class(CheckboxClass, clickable_cursor)
        .class(LabeledCheckboxClass, clickable_cursor)
        .class(RadioButtonClass, clickable_cursor)
        .class(LabeledRadioButtonClass, clickable_cursor)
        .class(ToggleButtonClass, clickable_cursor)
        .class(dropdown::DropdownClass, clickable_cursor)
        .class(ListItemClass, clickable_cursor)
        .class(slider::SliderClass, clickable_cursor)
        .class(ClickableClass, clickable_cursor)
}

fn clickable_cursor(style: Style) -> Style {
    style
        .cursor(CursorStyle::Pointer)
        .disabled(|style| style.cursor(CursorStyle::Default))
}
