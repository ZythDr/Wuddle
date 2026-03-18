//! AnchoredOverlay — shows an overlay pinned to the bottom-right of its underlay
//! using Iced's built-in overlay system (same mechanism used by pick_list).
//! The overlay has the exact screen position of the underlay widget, making
//! it immune to scroll offsets and layout estimation errors.

use iced::advanced::{
    layout::{self, Layout},
    mouse, overlay, renderer,
    widget::{tree, Tree, Widget},
    Clipboard, Shell,
};
use iced::{Element, Event, Length, Point, Rectangle, Size, Vector};

/// Wraps `underlay` and, when `is_open`, renders `overlay_content` as a
/// floating panel anchored to the bottom-right corner of the underlay.
///
/// Clicking outside the overlay publishes `on_dismiss` (if set).
pub struct AnchoredOverlay<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    underlay: Element<'a, Message, Theme, Renderer>,
    overlay_content: Element<'a, Message, Theme, Renderer>,
    is_open: bool,
    on_dismiss: Option<Message>,
}

impl<'a, Message, Theme, Renderer> AnchoredOverlay<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
    Message: Clone,
{
    pub fn new(
        underlay: impl Into<Element<'a, Message, Theme, Renderer>>,
        overlay_content: impl Into<Element<'a, Message, Theme, Renderer>>,
        is_open: bool,
    ) -> Self {
        Self {
            underlay: underlay.into(),
            overlay_content: overlay_content.into(),
            is_open,
            on_dismiss: None,
        }
    }

    pub fn on_dismiss(mut self, message: Message) -> Self {
        self.on_dismiss = Some(message);
        self
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for AnchoredOverlay<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
    Message: Clone + 'a,
    Theme: 'a,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    fn state(&self) -> tree::State {
        tree::State::None
    }

    fn children(&self) -> Vec<Tree> {
        vec![
            Tree::new(&self.underlay),
            Tree::new(&self.overlay_content),
        ]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.underlay, &self.overlay_content]);
    }

    fn size(&self) -> Size<Length> {
        self.underlay.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.underlay
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.underlay.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        )
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.underlay.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        )
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.underlay.as_widget().mouse_interaction(
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
        renderer: &Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        self.underlay
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        _viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        if !self.is_open {
            return self.underlay.as_widget_mut().overlay(
                &mut tree.children[0],
                layout,
                renderer,
                _viewport,
                translation,
            );
        }

        let bounds = layout.bounds();
        // Anchor = bottom-right corner of the underlay in absolute window coords.
        let anchor = Point::new(
            bounds.x + bounds.width + translation.x,
            bounds.y + bounds.height + translation.y,
        );
        // Pass the underlay's bounds so the overlay can skip dismissal when
        // the user clicks the button itself (ToggleMenu handles that case).
        let underlay_bounds = Rectangle {
            x: bounds.x + translation.x,
            y: bounds.y + translation.y,
            width: bounds.width,
            height: bounds.height,
        };

        Some(overlay::Element::new(Box::new(ContentOverlay {
            content: self.overlay_content.as_widget_mut(),
            tree: &mut tree.children[1],
            anchor,
            underlay_bounds,
            on_dismiss: self.on_dismiss.clone(),
        })))
    }
}

impl<'a, Message, Theme, Renderer> From<AnchoredOverlay<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer + 'a,
    Message: Clone + 'a,
    Theme: 'a,
{
    fn from(w: AnchoredOverlay<'a, Message, Theme, Renderer>) -> Self {
        Element::new(w)
    }
}

// ---------------------------------------------------------------------------
// Overlay implementation
// ---------------------------------------------------------------------------

struct ContentOverlay<'b, Message, Theme, Renderer> {
    content: &'b mut dyn Widget<Message, Theme, Renderer>,
    tree: &'b mut Tree,
    anchor: Point,
    underlay_bounds: Rectangle,
    on_dismiss: Option<Message>,
}

impl<'b, Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for ContentOverlay<'b, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
    Message: Clone,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        let limits = layout::Limits::new(Size::ZERO, bounds)
            .width(Length::Shrink)
            .height(Length::Shrink);
        let child = self.content.layout(self.tree, renderer, &limits);
        let size = child.size();

        // Right-align to anchor x; appear 2px below anchor y (visual gap).
        // Flip above if there is not enough room below.
        const GAP: f32 = 2.0;
        let x = (self.anchor.x - size.width).max(0.0);
        let y = if self.anchor.y + GAP + size.height > bounds.height {
            (self.anchor.y - size.height - 28.0 - GAP).max(0.0)
        } else {
            self.anchor.y + GAP
        };

        layout::Node::with_children(
            Size::new(bounds.width, bounds.height),
            vec![child.translate(Vector::new(x, y))],
        )
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        if let Some(child_layout) = layout.children().next() {
            self.content.draw(
                self.tree,
                renderer,
                theme,
                style,
                child_layout,
                cursor,
                &layout.bounds(),
            );
        }
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        // Dismiss on click outside the menu.
        // Skip dismiss when clicking the underlay button itself — ToggleMenu
        // handles that case and closes the menu without reopening it.
        if let Event::Mouse(mouse::Event::ButtonPressed(_)) = event {
            let cursor_pos = cursor.position().unwrap_or_default();
            let in_menu = layout
                .children()
                .next()
                .map(|l| l.bounds().contains(cursor_pos))
                .unwrap_or(false);
            let on_button = self.underlay_bounds.contains(cursor_pos);
            if !in_menu && !on_button {
                if let Some(msg) = &self.on_dismiss {
                    shell.publish(msg.clone());
                }
                return;
            }
        }

        // Forward to menu content.
        if let Some(child_layout) = layout.children().next() {
            self.content.update(
                self.tree,
                event,
                child_layout,
                cursor,
                renderer,
                clipboard,
                shell,
                &layout.bounds(),
            );
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if let Some(child_layout) = layout.children().next() {
            self.content
                .mouse_interaction(self.tree, child_layout, cursor, &layout.bounds(), renderer)
        } else {
            mouse::Interaction::None
        }
    }
}
