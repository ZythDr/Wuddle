# Iced 0.14 — Lessons Learned & Reference

Notes from porting Wuddle (a Tauri v2 app) to an Iced 0.14 native frontend.
This file documents API specifics, patterns that work, and pitfalls to avoid.

---

## Cargo Setup

```toml
iced = { version = "0.14", features = [
    "tokio",     # async runtime integration
    "canvas",    # Canvas widget for custom drawing
    "markdown",  # Markdown rendering widget
    "image",     # Image widget
    "svg",       # SVG widget
    "advanced",  # Required for custom widget / overlay implementation
] }
```

The `advanced` feature is **required** if you implement custom widgets via
`iced::advanced::Widget` or `iced::advanced::overlay::Overlay`.

---

## Application Entry Point

```rust
iced::application(App::new, App::update, App::view)
    .title("My App")
    .theme(App::theme)
    .subscription(App::subscription)
    .font(include_bytes!("../assets/fonts/MyFont.ttf"))
    .default_font(my_font)
    .window_size((1100.0, 850.0))
    .run()
```

- `App::new` returns `(App, Task<Message>)`
- `App::update` returns `Task<Message>`
- `App::view` returns `Element<'_, Message>`
- `App::theme` returns `Theme` (or your custom theme type)
- `App::subscription` returns `Subscription<Message>`

---

## Elm Architecture

Iced uses strict Elm architecture. Every UI interaction goes through:
1. A **Message** enum variant
2. `update(&mut self, msg: Message) -> Task<Message>`
3. `view(&self) -> Element<'_, Message>` rebuilds the entire UI on every update

`Task::none()` is the no-op return. Use `Task::perform(future, Msg)` for async.

---

## Layout System

### Length variants
```rust
Length::Fill          // expand to fill available space
Length::Shrink        // shrink to content size
Length::Fixed(f32)    // exact pixel size
Length::FillPortion(u16) // proportional fill
```

### Key layout widgets
```rust
column![a, b, c].spacing(8).padding([12, 16])
row![a, b, c].spacing(8).align_y(Alignment::Center)
container(content).width(Fill).height(Fill).center_x(Fill)
Space::new().width(Length::Fill)   // flexible spacer
Space::new().width(16).height(4)   // fixed spacer
scrollable(content).height(Fill)
```

### Padding syntax
```rust
.padding(16)              // all sides
.padding([8, 12])         // [vertical, horizontal]
.padding([top, right, bottom, left])
// Or use iced::Padding struct for asymmetric:
.padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 10.0 })
```

---

## Stack Widget (Layered Overlays)

`stack![]` renders layers on top of each other. All layers share the same
space; each layer is sized independently. **This is not a substitute for
proper overlays** (see below), but useful for topbar centering tricks etc.

```rust
use iced::widget::stack;

// Example: center tabs over left/right sections in a topbar
let sides = container(row![left, Space::new().width(Fill), right])
    .width(Fill).height(BAR_H).align_y(Center);
let center = container(tabs)
    .width(Fill).height(BAR_H).align_x(Center).align_y(Center);
let bar = stack![sides, center].width(Fill).height(BAR_H);
```

**Pitfall**: All stack layers must have identical `height()` set explicitly,
otherwise vertical alignment between layers will be off.

**Pitfall**: `stack![]` does NOT create proper overlays — content is clipped
to the stack's own bounds. For true floating overlays (context menus,
dropdowns), use the custom widget overlay system (see below).

---

## Proper Overlays — The `Widget::overlay()` System

This is how `pick_list` dropdowns work in Iced. A custom widget can return
an overlay via `overlay()` that renders on top of *everything*, anchored to
the widget's real screen position. This is the correct approach for context
menus, tooltips, dropdowns, etc.

### Why cursor-position or row-index estimation fails

Approaches like estimating overlay Y from `row_idx * row_height` compound
errors and don't account for scroll offsets. Tracking cursor Y via a
`MouseMoved` subscription is better but still imprecise. The overlay system
solves this by giving you **absolute window coordinates** of the widget.

### How it works

1. Implement `iced::advanced::Widget` for your wrapper widget.
2. In the `overlay()` method, `layout.bounds()` gives the widget's position
   in **absolute window coordinates** — exact, regardless of scroll.
3. Return an `overlay::Element` wrapping your `Overlay` impl.
4. The overlay's `layout()` receives the full window `Size` and must return
   a `layout::Node` with absolute coordinates (use `.translate()`).

### Iced 0.14 Widget trait — exact signatures

```rust
use iced::advanced::{
    layout::{self, Layout},
    mouse, overlay, renderer,
    widget::{tree, Tree, Widget},
    Clipboard, Shell,
};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for MyWidget {
    fn tag(&self) -> tree::Tag { tree::Tag::stateless() }
    fn state(&self) -> tree::State { tree::State::None }
    fn children(&self) -> Vec<Tree> { vec![] }
    fn diff(&self, tree: &mut Tree) { tree.diff_children(&[]); }
    fn size(&self) -> Size<Length> { ... }

    // Note: &mut self, not &self
    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer,
              limits: &layout::Limits) -> layout::Node { ... }

    fn draw(&self, tree: &Tree, renderer: &mut Renderer, theme: &Theme,
            style: &renderer::Style, layout: Layout<'_>,
            cursor: mouse::Cursor, viewport: &Rectangle) { ... }

    // Renamed from on_event in older versions; takes &Event (reference)
    fn update(&mut self, tree: &mut Tree, event: &Event, layout: Layout<'_>,
              cursor: mouse::Cursor, renderer: &Renderer,
              clipboard: &mut dyn Clipboard, shell: &mut Shell<'_, Message>,
              viewport: &Rectangle) { }

    fn mouse_interaction(&self, tree: &Tree, layout: Layout<'_>,
                         cursor: mouse::Cursor, viewport: &Rectangle,
                         renderer: &Renderer) -> mouse::Interaction {
        mouse::Interaction::None
    }

    fn operate(&mut self, tree: &mut Tree, layout: Layout<'_>,
               renderer: &Renderer,
               operation: &mut dyn iced::advanced::widget::Operation) { }

    // Extra viewport: &Rectangle param compared to older Iced versions
    fn overlay<'a>(&'a mut self, tree: &'a mut Tree, layout: Layout<'a>,
                   renderer: &Renderer, _viewport: &Rectangle,
                   translation: Vector)
        -> Option<overlay::Element<'a, Message, Theme, Renderer>> { None }
}
```

### Iced 0.14 Overlay trait — exact signatures

```rust
impl<Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for MyOverlay
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        // bounds = full window size
        // Return node in absolute window coordinates
        let child = self.content.layout(self.tree, renderer, &limits);
        layout::Node::with_children(
            Size::new(bounds.width, bounds.height),
            vec![child.translate(Vector::new(x, y))],
        )
    }

    fn draw(&self, renderer: &mut Renderer, theme: &Theme,
            style: &renderer::Style, layout: Layout<'_>,
            cursor: mouse::Cursor) { ... }

    // Renamed from on_event; takes &Event (reference); no return value
    fn update(&mut self, event: &Event, layout: Layout<'_>,
              cursor: mouse::Cursor, renderer: &Renderer,
              clipboard: &mut dyn Clipboard,
              shell: &mut Shell<'_, Message>) { }

    // Only 3 params — no viewport, no tree
    fn mouse_interaction(&self, layout: Layout<'_>, cursor: mouse::Cursor,
                         renderer: &Renderer) -> mouse::Interaction {
        mouse::Interaction::None
    }

    // No is_over() in Iced 0.14 — hit testing is done by the runtime
    // based on layout bounds. Position your layout node accurately.
}
```

**Important differences from Iced 0.13:**
- `on_event` → `update` (renamed, no return value, takes `&Event` not `Event`)
- `Widget::overlay` now takes an extra `_viewport: &Rectangle` parameter
- `Widget::layout` now takes `&mut self` not `&self`
- `tree::Tag` / `tree::State` live in `iced::advanced::widget::tree`, not `widget`
- `overlay::Overlay::mouse_interaction` has 3 params (no viewport/tree)
- `overlay::Overlay::is_over` **does not exist** in 0.14

### Context menu toggle pitfall

When using `AnchoredOverlay`, if you publish a dismiss message (`CloseMenu`)
for clicks outside the overlay, and the click lands on the underlay button
which fires `ToggleMenu`, both messages fire: `CloseMenu` sets state to None,
then `ToggleMenu` reopens it. Fix: check if the click is on the underlay and
skip the dismiss in that case, letting `ToggleMenu` handle it alone.

```rust
// In overlay update():
let on_button = self.underlay_bounds.contains(cursor_pos);
if !in_menu && !on_button {
    shell.publish(CloseMenu);
}
```

---

## Subscriptions

```rust
fn subscription(&self) -> Subscription<Message> {
    let mut subs = Vec::new();

    // Timer-based
    if self.some_flag {
        subs.push(
            iced::time::every(Duration::from_secs(60))
                .map(|_| Message::TimerTick),
        );
    }

    // Event listener (keyboard, mouse, window events)
    subs.push(iced::event::listen_with(|event, _status, _window| {
        match event {
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                Some(Message::MouseMoved(position))
            }
            _ => None,
        }
    }));

    Subscription::batch(subs)
}
```

**Note**: A continuous `CursorMoved` subscription generates a message on
every mouse movement. Prefer the overlay system over cursor tracking for
positioning UI elements.

---

## Async Tasks

```rust
// Fire-and-forget async
Task::perform(
    async move { my_async_fn(arg).await },
    Message::MyResult,
)

// Blocking work off the async thread
Task::perform(
    tokio::task::spawn_blocking(move || heavy_computation()),
    |res| Message::MyResult(res.unwrap()),
)
```

---

## Styling

Iced 0.14 uses closure-based styling:

```rust
button(content)
    .style(move |_theme, status| match status {
        button::Status::Hovered => my_hovered_style,
        button::Status::Pressed => my_pressed_style,
        _ => my_default_style,
    })

container(content)
    .style(move |_theme| container::Style {
        background: Some(Background::Color(color)),
        border: Border { color, width: 1.0, radius: 0.0.into() },
        ..Default::default()
    })
```

`ThemeColors` pattern: define a struct of `iced::Color` fields and copy it
into closures with `let c = *colors;` to avoid lifetime issues.

---

## Canvas / Custom Drawing (Spinner Example)

```rust
use iced::widget::canvas::{self, Canvas, Frame, Geometry};

struct SpinnerCanvas { tick: u32, color: iced::Color }

impl canvas::Program<Message> for SpinnerCanvas {
    type State = ();

    fn draw(&self, _state: &(), renderer: &Renderer, _theme: &Theme,
            bounds: Rectangle, _cursor: Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        // draw arcs, paths, etc.
        vec![frame.into_geometry()]
    }
}

// Usage:
canvas(SpinnerCanvas { tick, color }).width(26).height(26)
```

---

## Fixed-Width Centered Tab Buttons

To make all tabs the same width with centered text:

```rust
let content: Element<Message> = container(label)
    .width(Length::Fill)
    .center_x(Length::Fill)
    .into();

button(content)
    .on_press(msg)
    .padding([7, 0])          // horizontal padding = 0, width handles it
    .width(Length::Fixed(114.0))
```

Without wrapping the label in a `container` with `center_x`, the text will
be left-aligned inside a fixed-width button.

---

## Vertical Dividers in Tables

`rule::vertical` expands to fill all available height in scrollable layouts,
which breaks row sizing. Use a `container` with fixed height instead:

```rust
fn vdiv<'a>(alpha: f32, height: u32) -> Element<'a, Message> {
    container(Space::new().width(1).height(height))
        .width(1)
        .style(move |_| container::Style {
            background: Some(Background::Color(Color::from_rgba(1.0,1.0,1.0,alpha))),
            ..Default::default()
        })
        .into()
}
```

---

## Profile / Multi-Instance Pattern

Each profile uses a separate SQLite database:
- `default` profile → `wuddle.sqlite`
- Named profiles → `wuddle-{id}.sqlite`

Settings are persisted to `settings.json` in the app data dir. On startup,
scan for `wuddle-*.sqlite` files not tracked in settings to auto-discover
orphaned profiles.

---

## What Didn't Work

| Approach | Problem |
|----------|---------|
| Estimating overlay Y from `row_idx * row_height` | Compounds errors, breaks with scroll |
| Tracking `cursor_y` via `MouseMoved` subscription | Off-by-a-few-pixels, generates noise |
| `stack![]` for context menu overlays | Clipped to stack bounds, layout shifts |
| Inline context menu row expansion | Expands row height, pushes rows below down |
| `Length::Shrink` for fixed-count tab buttons | Tabs have inconsistent widths |
| `rule::vertical` in scrollable rows | Expands to fill height, breaks row sizing |

---

## What Worked

| Approach | Notes |
|----------|-------|
| `Widget::overlay()` for context menus | Exact pixel positioning, scroll-immune |
| `stack![]` for topbar centering | Tabs float independently of side sections |
| `container(label).center_x(Fill)` in fixed-width buttons | Centers text reliably |
| `tree::Tag::stateless()` for no-state custom widgets | Correct default for wrappers |
| `layout::Node::with_children(window_size, vec![child.translate(offset)])` | Correct overlay absolute positioning |
| Passing `underlay_bounds` to overlay to skip dismiss on button click | Fixes toggle reopen bug |
