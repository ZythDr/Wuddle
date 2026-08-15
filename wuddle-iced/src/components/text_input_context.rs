//! Reusable native-style context menus for editable single-line text fields.

use crate::anchored_overlay::AnchoredOverlay;
use crate::message::{TextInputAction, TextInputContext};
use crate::theme::{self, ThemeColors};
use crate::{App, Message};
use iced::advanced::{
    layout::{self, Layout},
    mouse, overlay, renderer,
    widget::{tree, Tree, Widget},
    Clipboard, Shell,
};
use iced::widget::text_input::{self, Value};
use iced::widget::{column, container, TextInput};
use iced::{Element, Event, Length, Padding, Pixels, Point, Rectangle, Size, Vector};

pub(crate) fn selected_text(context: &TextInputContext) -> Option<String> {
    if context.secure {
        return None;
    }
    let (start, end) = context.selection?;
    Some(Value::new(&context.value).select(start, end).to_string())
}

pub(crate) fn paste_message(
    context: &TextInputContext,
    clipboard_text: &str,
) -> Option<(Message, usize)> {
    let action = context.action.as_ref()?;
    let value = Value::new(&context.value);
    let (start, end) = context
        .selection
        .unwrap_or((context.cursor, context.cursor));
    let start = start.min(value.len());
    let end = end.min(value.len());
    let content: String = clipboard_text
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    let inserted = Value::new(&content).len();
    let updated = format!(
        "{}{}{}",
        value.select(0, start),
        content,
        value.select(end, value.len())
    );
    Some((action.apply(updated), start + inserted))
}

/// Build a text input with Wuddle's shared right-click Copy/Paste menu.
pub fn context_text_input<'a>(
    app: &'a App,
    colors: ThemeColors,
    key: impl Into<String>,
    placeholder: &str,
    value: &str,
) -> ContextTextInput<'a> {
    ContextTextInput::new(
        app.text_input_context.as_ref(),
        colors,
        key.into(),
        placeholder,
        value,
    )
}

pub struct ContextTextInput<'a> {
    input: TextInput<'a, Message>,
    context: Option<&'a TextInputContext>,
    colors: ThemeColors,
    key: String,
    value: String,
    widget_id: iced::widget::Id,
    action: Option<TextInputAction>,
    secure: bool,
}

impl<'a> ContextTextInput<'a> {
    fn new(
        context: Option<&'a TextInputContext>,
        colors: ThemeColors,
        key: String,
        placeholder: &str,
        value: &str,
    ) -> Self {
        let widget_id = iced::widget::Id::from(format!("text-input-context:{key}"));
        Self {
            input: TextInput::new(placeholder, value).id(widget_id.clone()),
            context,
            colors,
            key,
            value: value.to_string(),
            widget_id,
            action: None,
            secure: false,
        }
    }

    pub fn id(mut self, id: impl Into<iced::widget::Id>) -> Self {
        self.widget_id = id.into();
        self.input = self.input.id(self.widget_id.clone());
        self
    }

    pub fn secure(mut self, secure: bool) -> Self {
        self.secure = secure;
        self.input = self.input.secure(secure);
        self
    }

    pub fn on_input(mut self, action: impl Fn(String) -> Message + Send + Sync + 'static) -> Self {
        let action = TextInputAction::new(action);
        let input_action = action.clone();
        self.input = self.input.on_input(move |value| input_action.apply(value));
        self.action = Some(action);
        self
    }

    pub fn on_submit(mut self, message: Message) -> Self {
        self.input = self.input.on_submit(message);
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.input = self.input.width(width);
        self
    }

    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.input = self.input.padding(padding);
        self
    }

    #[allow(dead_code)]
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.input = self.input.size(size);
        self
    }
}

impl<'a> From<ContextTextInput<'a>> for Element<'a, Message> {
    fn from(input: ContextTextInput<'a>) -> Self {
        let is_open = input
            .context
            .is_some_and(|context| context.key == input.key);
        let anchor = input
            .context
            .filter(|context| context.key == input.key)
            .map(|context| context.position)
            .unwrap_or(Point::ORIGIN);

        let copy_available = input.context.is_some_and(|context| {
            context.key == input.key && !context.secure && context.selection.is_some()
        });
        let paste_available = input
            .context
            .is_some_and(|context| context.key == input.key && context.action.is_some());

        let copy = if copy_available {
            crate::ctx_menu_item("Copy", Message::CopyTextInputSelection, input.colors)
        } else {
            crate::ctx_menu_item_disabled("Copy", input.colors)
        };
        let paste = if paste_available {
            crate::ctx_menu_item("Paste", Message::PasteIntoTextInput, input.colors)
        } else {
            crate::ctx_menu_item_disabled("Paste", input.colors)
        };
        let colors = input.colors;
        let menu = container(column![copy, paste].spacing(2))
            .padding(6)
            .width(140)
            .style(move |_theme| theme::context_menu_style(colors));

        let underlay = SelectionAwareInput {
            input: input.input.into(),
            key: input.key,
            value: input.value,
            widget_id: input.widget_id,
            action: input.action,
            secure: input.secure,
        };

        AnchoredOverlay::new(underlay, menu, is_open)
            .at_point(anchor)
            .dismiss_on_underlay_click(true)
            .on_dismiss(Message::CloseTextInputContext)
            .into()
    }
}

struct SelectionAwareInput<'a> {
    input: Element<'a, Message>,
    key: String,
    value: String,
    widget_id: iced::widget::Id,
    action: Option<TextInputAction>,
    secure: bool,
}

impl<'a> From<SelectionAwareInput<'a>> for Element<'a, Message> {
    fn from(input: SelectionAwareInput<'a>) -> Self {
        Element::new(input)
    }
}

impl Widget<Message, iced::Theme, iced::Renderer> for SelectionAwareInput<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    fn state(&self) -> tree::State {
        tree::State::None
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.input)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.input]);
    }

    fn size(&self) -> Size<Length> {
        self.input.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.input
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.input.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.input.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        if !matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right))
        ) {
            return;
        }
        let Some(position) = cursor.position_in(layout.bounds()) else {
            return;
        };

        type Paragraph = <iced::Renderer as iced::advanced::text::Renderer>::Paragraph;
        let value = Value::new(&self.value);
        let state = tree.children[0]
            .state
            .downcast_ref::<text_input::State<Paragraph>>();
        let cursor_state = state.cursor().state(&value);
        let cursor_index = match cursor_state {
            text_input::cursor::State::Index(index) => index,
            text_input::cursor::State::Selection { end, .. } => end.min(value.len()),
        };

        shell.publish(Message::OpenTextInputContext(TextInputContext {
            key: self.key.clone(),
            value: self.value.clone(),
            selection: state.cursor().selection(&value),
            cursor: cursor_index,
            position,
            widget_id: self.widget_id.clone(),
            action: self.action.clone(),
            secure: self.secure,
        }));
        shell.capture_event();
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.input.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        self.input
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, iced::Theme, iced::Renderer>> {
        self.input.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{paste_message, selected_text};
    use crate::message::{TextInputAction, TextInputContext};
    use crate::Message;
    use iced::Point;

    fn context(value: &str, selection: Option<(usize, usize)>, cursor: usize) -> TextInputContext {
        TextInputContext {
            key: "test".into(),
            value: value.into(),
            selection,
            cursor,
            position: Point::ORIGIN,
            widget_id: iced::widget::Id::new("test"),
            action: Some(TextInputAction::new(Message::SetProjectSearch)),
            secure: false,
        }
    }

    #[test]
    fn copy_uses_only_the_selected_graphemes() {
        let context = context("before 🍪 after", Some((7, 8)), 8);
        assert_eq!(selected_text(&context).as_deref(), Some("🍪"));
    }

    #[test]
    fn secure_fields_never_offer_copy_content() {
        let mut context = context("secret", Some((0, 6)), 6);
        context.secure = true;
        assert_eq!(selected_text(&context), None);
    }

    #[test]
    fn paste_replaces_selection_and_strips_single_line_controls() {
        let context = context("before OLD after", Some((7, 10)), 10);
        let (message, cursor) = paste_message(&context, "new\nvalue").unwrap();
        assert_eq!(cursor, 15);
        assert!(
            matches!(message, Message::SetProjectSearch(value) if value == "before newvalue after")
        );
    }

    #[test]
    fn paste_inserts_at_the_saved_caret() {
        let context = context("before after", None, 7);
        let (message, cursor) = paste_message(&context, "new ").unwrap();
        assert_eq!(cursor, 11);
        assert!(matches!(message, Message::SetProjectSearch(value) if value == "before new after"));
    }

    #[test]
    fn context_debug_output_redacts_field_contents() {
        let context = context("never-log-this-value", Some((0, 5)), 5);
        let debug = format!("{context:?}");
        assert!(!debug.contains("never-log-this-value"));
        assert!(debug.contains("<redacted>"));
    }
}
