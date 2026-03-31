# Iced 0.14 — Wuddle Development Reference

Discoveries made while porting the Tauri/Svelte Wuddle frontend to Iced 0.14.

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

### Font colors: text vs text_soft vs muted

| Tier | Usage | ThemeColors field |
|------|-------|-------------------|
| Bright | File names, headings, interactive text | `colors.text` |
| Soft | Body text, descriptions, README, stats | `colors.text_soft` |
| Dim | Labels, hints, placeholders | `colors.muted` |

Use `text_soft` for anything readable but not prominent. Use `muted` only for labels.

---

## Stack Widget (Layered Overlays)

`stack![]` renders layers on top of each other. All layers share the same
space; each layer is sized independently.

```rust
// Example: center tabs over left/right sections in a topbar
let sides = container(row![left, Space::new().width(Fill), right])
    .width(Fill).height(BAR_H).align_y(Center);
let center = container(tabs)
    .width(Fill).height(BAR_H).align_x(Center).align_y(Center);
let bar = stack![sides, center].width(Fill).height(BAR_H);
```

**Pitfall**: All stack layers must have identical `height()` set explicitly.

**Pitfall**: `stack![]` does NOT create proper overlays — content is clipped
to the stack's own bounds. For true floating overlays, use the overlay system.

---

## Proper Overlays — The `Widget::overlay()` System

This is how `pick_list` dropdowns work in Iced. A custom widget can return
an overlay that renders on top of *everything*, anchored to exact screen position.

### Why cursor-position or row-index estimation fails

Approaches like estimating overlay Y from `row_idx * row_height` compound
errors and don't account for scroll offsets. The overlay system gives you
**absolute window coordinates** of the widget.

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

    // Renamed from on_event; takes &Event (reference)
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
        layout::Node::with_children(
            Size::new(bounds.width, bounds.height),
            vec![child.translate(Vector::new(x, y))],
        )
    }

    fn draw(&self, renderer: &mut Renderer, theme: &Theme,
            style: &renderer::Style, layout: Layout<'_>,
            cursor: mouse::Cursor) { ... }

    // Renamed from on_event; no return value
    fn update(&mut self, event: &Event, layout: Layout<'_>,
              cursor: mouse::Cursor, renderer: &Renderer,
              clipboard: &mut dyn Clipboard,
              shell: &mut Shell<'_, Message>) { }

    // Only 3 params — no viewport, no tree
    fn mouse_interaction(&self, layout: Layout<'_>, cursor: mouse::Cursor,
                         renderer: &Renderer) -> mouse::Interaction {
        mouse::Interaction::None
    }

    // No is_over() in Iced 0.14 — hit testing done by the runtime
}
```

**Important differences from Iced 0.13:**
- `on_event` → `update` (renamed, no return value, takes `&Event` not `Event`)
- `Widget::overlay` now takes an extra `_viewport: &Rectangle` parameter
- `Widget::layout` now takes `&mut self` not `&self`
- `overlay::Overlay::mouse_interaction` has 3 params (no viewport/tree)
- `overlay::Overlay::is_over` **does not exist** in 0.14

### Context menu toggle pitfall

When a dismiss message (`CloseMenu`) and toggle message both fire on the same
click, the menu reopens. Fix: check if click is on the underlay button:

```rust
// In overlay update():
let on_button = self.underlay_bounds.contains(cursor_pos);
if !in_menu && !on_button {
    shell.publish(CloseMenu);
}
```

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
        vec![frame.into_geometry()]
    }
}

// Usage:
canvas(SpinnerCanvas { tick, color }).width(26).height(26)
```

**Always reserve spinner space** so layout doesn't shift when busy state changes:
```rust
if self.is_busy() {
    canvas(SpinnerCanvas { tick, color }).width(26).height(26).into()
} else {
    Space::new().width(26).height(26).into()  // same size, invisible
}
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

Without wrapping in a `container` with `center_x`, text will be left-aligned.

### Icon-width tabs (SVG or Unicode glyph)

For 32px-wide icon tabs use `Length::Fixed(32.0)`. For Unicode glyphs, set
`line_height(1.0)` so the text height equals the font size exactly — otherwise
the button will be taller than SVG icon buttons:

```rust
container(text("ⓘ").size(17).color(icon_color).line_height(1.0))
    .center_x(Length::Fill)
```

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

## Markdown Widget

### API overview

```rust
// Option A: parse into items (simple, for storing in state)
let items: Vec<iced::widget::markdown::Item> = iced::widget::markdown::parse(&text).collect();

// Option B: parse via Content (preferred — also exposes image URLs)
let content = iced::widget::markdown::Content::parse(&text);
let items: Vec<iced::widget::markdown::Item> = content.items().to_vec();
let image_urls: &HashSet<String> = content.images(); // exact URLs iced will render

// Render without custom viewer (links become OpenUrl messages via .map)
iced::widget::markdown::view(&items, settings)
    .map(Message::OpenUrl)

// Render with custom viewer (e.g. for image rendering from cache)
iced::widget::markdown::view_with(&items, settings, &my_viewer)
```

`markdown::Uri` is a type alias for `String`.

### Settings

```rust
let settings = iced::widget::markdown::Settings::with_text_size(
    13,
    iced::widget::markdown::Style::from(&self.theme()),
);
// Heading sizes are derived automatically (h1 = 2× base, h2 = 1.75×, etc.)
```

`Settings` also implements `From<&Theme>`, so `iced::widget::markdown::Settings::from(&theme)`
works as a quick default.

### Item enum (relevant variants)

```rust
pub enum Item {
    Heading(pulldown_cmark::HeadingLevel, Text),
    Paragraph(Text),
    CodeBlock { language: Option<String>, code: String, lines: Vec<Text> },
    List { start: Option<u64>, bullets: Vec<Bullet> },
    Image { url: Uri, title: String, alt: Text },
    Quote(Vec<Item>),
    Rule,
    Table { /* ... */ },
}
```

`Item` is `Debug + Clone` and owns all its data — safe to store in app state.

### Custom Viewer for images

The default viewer renders image alt text as placeholder. To display actual
images pre-fetched into a cache, implement the `Viewer` trait:

```rust
struct ImageViewer<'a> {
    cache: &'a HashMap<String, Vec<u8>>,
    raw_base_url: &'a str,
}

impl<'a> iced::widget::markdown::Viewer<'a, Message> for ImageViewer<'a> {
    fn on_link_click(url: String) -> Message {
        Message::OpenUrl(url)
    }

    fn image(
        &self,
        _settings: iced::widget::markdown::Settings,
        url: &'a String,
        _title: &'a str,
        _alt: &iced::widget::markdown::Text,
    ) -> Element<'a, Message> {
        let bytes = self.cache.get(url.as_str()).or_else(|| {
            let abs = resolve_image_url(url, self.raw_base_url);
            self.cache.get(abs.as_str())
        });
        if let Some(bytes) = bytes {
            container(
                iced::widget::image(iced::widget::image::Handle::from_bytes(bytes.clone()))
                    .width(Length::Fill),
            )
            .width(Length::Fill)
            .padding([4, 0])
            .into()
        } else {
            container(
                text(format!("[image: {}]", url.split('/').last().unwrap_or(url)))
                    .size(11)
                    .color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.25)),
            )
            .padding([2, 0])
            .into()
        }
    }
}

// Usage:
let viewer = ImageViewer { cache: &preview.image_cache, raw_base_url: &preview.raw_base_url };
let settings = iced::widget::markdown::Settings::with_text_size(13, Style::from(&theme));
iced::widget::markdown::view_with(&items, settings, &viewer)
```

**Lifetime rule:** `ImageViewer<'a>` must have its cache reference live at
least as long as `'a`. Borrow from app state (e.g. `self.add_repo_preview`),
not from a local variable.

### Storing parsed items in state

**Problem:** `markdown::view` borrows the items and the returned `Element` has
the same lifetime. Parsing inside the view function creates a local `Vec<Item>`
that is dropped before the element is used.

**Solution:** Store `Vec<Item>` in app state. Since `Item` is `Clone` and owns
all its string data, this is straightforward:

```rust
pub struct RepoPreviewInfo {
    pub readme_items: Vec<iced::widget::markdown::Item>,
    // ...
}
// Parse once when fetching:
let content = iced::widget::markdown::Content::parse(&readme_text);
let readme_items = content.items().to_vec();
```

### Correct image URL collection (critical)

**Problem:** A custom regex scanner to collect image URLs may extract different
strings than what iced's markdown renderer passes to `ImageViewer::image()`.
This causes cache misses even when images were successfully fetched.

**Root cause:** iced uses pulldown-cmark internally to parse markdown. If your
URL scanner uses a different parser or regex, the extracted URLs may differ
(e.g. reference-style images `![alt][ref]`, different whitespace handling, etc.)

**Solution:** Use `Content::parse().images()` to get the exact set of image
URLs that iced will render. These are guaranteed to match because they come from
the same pulldown-cmark parse pass:

```rust
let content = iced::widget::markdown::Content::parse(&readme_text);
let image_urls: Vec<String> = content.images().iter().cloned().collect();
// Also collect HTML <img src="..."> tags separately (not handled by pulldown-cmark):
for url in collect_html_img_urls(&readme_text) {
    if !image_urls.contains(&url) { image_urls.push(url); }
}
let image_cache = fetch_images(client, &image_urls, &raw_base).await;
```

### GIF / animated images

Iced 0.14 does **not** support animated GIFs. `image::Handle::from_bytes()`
displays the first frame only. Canvas-based animation would be required.

---

## Image Loading

### `image::Handle::from_bytes`

```rust
let handle = iced::widget::image::Handle::from_bytes(bytes.clone());
iced::widget::image(handle).width(Length::Fill)
```

`from_bytes` accepts any `Into<Bytes>` — `Vec<u8>` works directly.
Supports PNG, JPEG, WebP, BMP, and most common formats.

### Fetching remote images

Use `reqwest` with tokio timeouts and size limits:

```rust
// Limits: max 12 images, 5 MB each, 20 MB total
for url in image_urls.iter().take(12) {
    let abs_url = resolve_image_url(url, raw_base_url);
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        client.get(&abs_url).send(),
    ).await;
    if let Ok(Ok(resp)) = result {
        if resp.status().is_success() {
            if let Ok(bytes) = resp.bytes().await {
                if bytes.len() <= 5_000_000 {
                    // Store by both original and absolute URL for flexible lookup
                    cache.insert(url.clone(), bytes.to_vec());
                    if abs_url != *url { cache.insert(abs_url, bytes.to_vec()); }
                }
            }
        }
    }
}
```

### Resolving relative image URLs

```rust
fn resolve_image_url(url: &str, raw_base_url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        let clean = url.trim_start_matches("./").trim_start_matches('/');
        format!("{}{}", raw_base_url, clean)
    }
}
```

Raw base URLs by forge:
- GitHub: `https://raw.githubusercontent.com/{owner}/{repo}/HEAD/`
- GitLab: `https://gitlab.com/{owner}/{repo}/raw/HEAD/`
- Gitea: `{scheme}://{host}/{owner}/{repo}/raw/branch/master/`

---

## Subscriptions

```rust
fn subscription(&self) -> Subscription<Message> {
    let mut subs = Vec::new();

    // Timer-based
    if self.opt_auto_check {
        let mins = self.auto_check_minutes.max(1) as u64;
        subs.push(
            iced::time::every(Duration::from_secs(mins * 60))
                .map(|_| Message::AutoCheckTick),
        );
    }

    // Spinner animation (only when busy)
    if self.is_busy() {
        subs.push(
            iced::time::every(Duration::from_millis(80))
                .map(|_| Message::SpinnerTick),
        );
    }

    // Keyboard events (gated — only active when dialog is open)
    if self.dialog.is_some() {
        subs.push(iced::event::listen_with(|event, _status, _window| {
            match event {
                iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                    ..
                }) => Some(Message::CloseDialog),
                _ => None,
            }
        }));
    }

    Subscription::batch(subs)
}
```

**Note:** A continuous `CursorMoved` subscription generates a message on
every mouse movement. Prefer the overlay system over cursor tracking.

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

## Font Loading

```rust
iced::application(...)
    .font(include_bytes!("../assets/fonts/LifeCraft_Font.ttf"))
    .font(include_bytes!("../assets/fonts/FrizQuadrataStd-Regular.otf"))
    .default_font(if use_friz { FRIZ } else { Font::DEFAULT })

const LIFECRAFT: Font = Font::with_name("LifeCraft");
const FRIZ: Font = Font::with_name("Friz Quadrata Std");
```

The font name must match the font's internal name metadata, not the filename.

**Note:** `default_font` is set once at startup. Changing it at runtime requires
a restart. Log a message to tell the user when they toggle the font setting.

---

## Pick List with String Items

```rust
let modes = vec!["auto".to_string(), "addon".to_string(), "dll".to_string()];
iced::widget::pick_list(modes, Some(current_mode.clone()), |new_mode: String| {
    Message::OpenDialog(Dialog::AddRepo { mode: new_mode, ..rest })
})
```

`T` must be `ToString + PartialEq + Clone + 'static`. `Vec<String>` works directly.

---

## Dialog Pattern

### Dialog enum with inline field state

Fields like URL input, mode, and toggle state live directly in the `Dialog` enum
variant. This keeps all dialog-related state together and avoids scattered App fields.

For state that must survive dialog-internal navigation (e.g. a fetched preview),
store it in `App` rather than in `Dialog`:

```rust
// In App:
pub add_repo_preview: Option<RepoPreviewInfo>,  // survives re-opening dialog
// URL input gets its own message to avoid resetting the preview:
Message::SetAddRepoUrl(String)  // updates url field + fires preview fetch
```

### Two-card floating side panel layout

When a preview is loaded, switch from single-card to two side-by-side cards:

```
+--sidebar (260px)-------+  +--main form (fills)----------+
| About                  |  | Title                  [×]  |
| Atlas-TW               |  | URL input                   |
| ★ 16   ⑂ 1            |  | ┌──README (scrollable)────┐  |
| Language   Lua         |  | │ Heading                  │  |
| ────────────────        |  | │ Images render inline     │  |
| Files                  |  | │ Text continues...        │  |
| ▸ Images/              |  | └──────────────────────────┘  |
| ▸ Locales/             |  | [Release Notes] [Cancel] [Add]|
+------------------------+  +-------------------------------+
```

- Detect `has_two_cards` in `view()`, give `dialog_box` `width(Fill).height(Fill)`
- `view_dialog()` returns `row![sidebar, form_card].width(Fill).height(Fill)` — each card fills the row
- Sidebar: 260px fixed width; form card: `width(Fill).height(Fill)`

### Making dialogs fill ~90% of the window

Add uniform padding to the `centered_dialog` container. The padding creates margins on all
sides, and `Fill` children fill the remaining area up to those margins:

```rust
let centered_dialog = container(dialog_blocker)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .padding(40);  // ~40px margin ≈ 90% on a typical 800px window

// For the dialog box itself (AddRepo etc.):
container(content)
    .width(Length::Fill)
    .height(Length::Fill)  // fills the padded space
    .into()

// Inner scrollable content also needs height(Fill) to fill dialog height
row![sidebar_card, form_card]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
```

**Key rule:** `center_y(Fill)` with symmetric padding still centers compact dialogs
(those with `height(Shrink)`) in the window. Only `height(Fill)` dialogs stretch to fill it.
So compact confirmation dialogs remain centered without changes.

**Conditional fill height:** for cases where the same dialog can be compact (URL entry mode)
or tall (quick-add list mode), check the condition before the `dialog_box` construction:

```rust
let use_fill_height = match dialog {
    Dialog::AddRepo { url, is_addons, .. } => !is_addons && url.trim().is_empty(),
    _ => false,
};
// Then set height(Fill) on dialog_box only when use_fill_height is true.
```

### Click-through prevention

**Critical:** `stack![]` dispatches events to ALL layers. A `mouse_area` alone
cannot stop lower-layer widgets from firing when a dialog overlay is shown.

**Solution:** Wrap the entire overlay in `iced::widget::opaque()`:

```rust
let overlay = iced::widget::opaque(
    stack![scrim, centered_dialog]
        .width(Length::Fill)
        .height(Length::Fill),
);
stack![main_content, overlay]
```

`opaque()` returns `Status::Captured` for every event, making the overlay a
complete event sink. Available via `iced::widget::opaque` (re-exported from helpers).

---

## Profile / Multi-Instance Pattern

Each profile uses a separate SQLite database:
- `default` profile → `wuddle.sqlite`
- Named profiles → `wuddle-{id}.sqlite`

Settings are persisted to `settings.json` in the app data dir.

---

## Forge / GitHub API Patterns

### GitHub API endpoints

- Repo info: `GET https://api.github.com/repos/{owner}/{repo}`
  - Headers: `Accept: application/vnd.github+json`
  - Auth: `Authorization: Bearer {token}` (optional but increases rate limits)
- README (raw text): `GET https://api.github.com/repos/{owner}/{repo}/readme`
  - Headers: `Accept: application/vnd.github.raw+json`
- File tree: `GET https://api.github.com/repos/{owner}/{repo}/contents/`
- Releases: `GET https://api.github.com/repos/{owner}/{repo}/releases?per_page=20`

### GitLab API endpoints

- Repo info: `GET https://gitlab.com/api/v4/projects/{owner%2Frepo}`
- README: `GET https://gitlab.com/{owner}/{repo}/raw/HEAD/README.md`
- File tree: `GET https://gitlab.com/api/v4/projects/{owner%2Frepo}/repository/tree?per_page=50`
- Releases: `GET https://gitlab.com/api/v4/projects/{owner%2Frepo}/releases`

### Gitea / Forgejo / Codeberg API endpoints

- Repo info: `GET {scheme}://{host}/api/v1/repos/{owner}/{repo}`
- README: `GET {scheme}://{host}/{owner}/{repo}/raw/branch/master/README.md`
- File tree: `GET {scheme}://{host}/api/v1/repos/{owner}/{repo}/contents/`
- Releases: `GET {scheme}://{host}/api/v1/repos/{owner}/{repo}/releases?limit=20`

---

## Equal-Height Cards in a Row (CSS `align-items: stretch` Equivalent)

Iced has no direct equivalent of CSS `align-items: stretch`. To make sibling cards in a
`row![]` match the height of the tallest card:

### The pattern

Give the **naturally taller card** `height(Shrink)` (default) — it anchors the row height.
Give the **shorter card(s)** `height(Fill)` — they stretch to match.

How it works: a `Row` with `height(Shrink)` sets its cross-axis height to the max natural
height of all its Shrink children. Fill children then receive that same height.

```rust
fn settings_card<'a>(content: impl Into<Element<'a, Message>>, colors: &ThemeColors)
    -> Element<'a, Message>
{
    let c = *colors;
    container(container(content).padding(16))
        .width(Length::Fill)
        // height defaults to Shrink — this is the "anchor" card
        .style(move |_theme| theme::card_style(&c))
        .into()
}

fn settings_card_fill<'a>(content: impl Into<Element<'a, Message>>, colors: &ThemeColors)
    -> Element<'a, Message>
{
    let c = *colors;
    container(container(content).padding(16))
        .width(Length::Fill)
        .height(Length::Fill)  // stretches to match the tallest sibling
        .style(move |_theme| theme::card_style(&c))
        .into()
}

// Usage: identify which card is naturally taller in each row
let rendering = settings_card(rendering_col, &c);     // taller — Shrink anchor
let camera    = settings_card_fill(camera_col, &c);   // shorter — Fill stretch

let audio  = settings_card(audio_col, &c);            // taller — Shrink anchor
let system = settings_card_fill(system_col, &c);      // shorter — Fill stretch

row![rendering, camera].spacing(8).width(Length::Fill)   // both cards same height ✓
row![audio, system].spacing(8).width(Length::Fill)
```

### What does NOT work

- **Manual spacers** (`Space::new().height(X)`) are fragile — they break when fonts,
  padding, or content changes, and require re-tuning after every content edit.
- **`height(Fill)` on ALL cards** in a row with `height(Shrink)` — Fill children
  contribute 0 to the row's natural height, so the row may collapse.
- **Wrapping in a fixed-height container** — requires hardcoded pixel heights that
  drift when content changes.

### Equal-height cards when there is only one row (e.g. About page)

When each column already has the same number of items except one is shorter, add an
explicit `Space::new().height(X)` to the shorter column where:

```
X = (height_of_taller - height_of_shorter) - (one_column_spacing_gap)
```

Use visual inspection (screenshots) to confirm: a 1-row height difference ≈ `text_size * 1.3 + gap`.
This is accurate enough when content is stable.

---

## Overlay Scrollbar (No Content Shift)

By default, an Iced `scrollable` with a vertical scrollbar reserves width for the
scrollbar (typically 10px + 8px gap = 18px), causing content to shift left when a
scrollbar appears. This misaligns table columns and header rows.

### Solution: overlay scrollbar

Use `Scrollbar::new()` **without** `.spacing()` — the scrollbar floats over the content:

```rust
pub fn vscroll_overlay() -> iced::widget::scrollable::Direction {
    iced::widget::scrollable::Direction::Vertical(
        iced::widget::scrollable::Scrollbar::new()
            .width(10)
            .scroller_width(10),
        // No .spacing() — scrollbar is an overlay, does not shift content
    )
}

// vs. vscroll() which reserves space:
pub fn vscroll() -> iced::widget::scrollable::Direction {
    iced::widget::scrollable::Direction::Vertical(
        iced::widget::scrollable::Scrollbar::new()
            .width(10)
            .scroller_width(10)
            .spacing(8),  // adds 8px gap, shifts content left by 18px total
    )
}
```

Use `vscroll_overlay()` for tables and any scrollable where column alignment must be
consistent regardless of scrollbar presence. Use `vscroll()` where extra right padding
is acceptable.

**Do NOT add VSCROLL_RESERVED padding to header rows** when using overlay mode — the
scrollbar floats and headers stay aligned automatically.

---

## Inline Clear Button in Text Input

The pattern for an "×" clear button that appears inside a text input, right-aligned,
only when the field has text (matching browser-native input behavior):

```rust
let show_clear = !value.is_empty();

stack![
    text_input(placeholder, value)
        .on_input(Message::SetValue)
        .padding(iced::Padding {
            top: 4.0,
            right: if show_clear { 26.0 } else { 10.0 },  // reserve space for X
            bottom: 4.0,
            left: 10.0,
        })
        .width(Length::Fill),

    {   // Typed variable required — type inference fails on Space::new().into() in if/else
        let clear_el: Element<Message> = if show_clear {
            button(text("\u{2715}").size(12).color(colors.muted))
                .on_press(Message::SetValue(String::new()))
                .padding([3, 7])
                .style(move |_t, _s| button::Style {
                    background: None,
                    text_color: colors.muted,
                    border: iced::Border::default(),
                    shadow: iced::Shadow::default(),
                    snap: true,
                })
                .into()
        } else {
            Space::new().into()
        };
        container(clear_el)
    }
    .width(Length::Fill)
    .height(Length::Fill)   // REQUIRED: without this, container has Shrink height
    .align_x(iced::Alignment::End)
    .align_y(iced::Alignment::Center)
    .padding(iced::Padding { top: 0.0, right: 4.0, bottom: 0.0, left: 0.0 }),
]
.width(Length::Fill)
```

**Critical: `height(Length::Fill)` on the overlay container**

Inside a `stack![]`, each layer occupies the same bounding box (the stack's computed size).
The stack's height = max natural height of all Shrink children = the text input height.

If the overlay container has `height(Shrink)` (default), it shrinks to the button size
and is positioned at `(0, 0)` — the **top-left** of the stack area. `align_y(Center)` has
no room to center anything. Result: button appears in the top-right corner.

With `height(Length::Fill)`, the container fills the stack height (= input height), and
`align_y(Center)` correctly centers the button vertically within the input.

---

## Bold Font Loading

Iced automatically selects font weight variants when multiple weights of the same family
are registered. To get proper bold rendering (e.g. in markdown headers):

```rust
// Register both Regular and Bold under the same family name
iced::application(...)
    .font(include_bytes!("../assets/fonts/NotoSans-Regular.ttf"))
    .font(include_bytes!("../assets/fonts/NotoSans-Bold.ttf"))
    .default_font(Font::with_name("Noto Sans"))
```

Iced will automatically use `NotoSans-Bold.ttf` when a widget requests
`Font { weight: Weight::Bold, family: Family::Name("Noto Sans"), .. }`.
Without the bold variant registered, Iced fakes bold (thicker rendering) or
uses regular weight everywhere.

### Font family identity check pitfall

When checking whether a font is a specific named font, do NOT compare `font.family`:

```rust
// WRONG — Font::with_name("Noto Sans") has Family::Name("Noto Sans"),
// not Family::SansSerif, so this check always fails:
if colors.body_font.family == iced::font::Family::SansSerif { ... }

// CORRECT — compare the full Font struct:
if colors.body_font == FRIZ { ... }
// Or compare the specific font constant you care about:
const FRIZ: Font = Font::with_name("Friz Quadrata Std");
fn name_font(colors: &ThemeColors) -> Font {
    if colors.body_font == FRIZ {
        FRIZ  // Friz doesn't have a bold variant
    } else {
        Font { weight: iced::font::Weight::Bold, ..colors.body_font }
    }
}
```

---

## Markdown Link Color

`iced::widget::markdown::Style` has a `link_color` field that defaults to the theme's
primary/accent color. To match your app's link color scheme, override it:

```rust
let mut md_style = iced::widget::markdown::Style::from(&self.theme());
md_style.link_color = colors.link;  // use your ThemeColors.link field
let settings = iced::widget::markdown::Settings::with_text_size(13, md_style);
```

Apply this to **all** markdown views in the app (README, release notes, changelog, etc.)
for consistent link styling.

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
| Custom regex scanner for markdown image URLs | Mismatches iced's pulldown-cmark URLs |
| Unicode glyph tab icon at large `size()` | Button becomes taller than SVG icon tabs |
| Manual `Space::new().height(X)` to match card heights | Fragile — breaks on any content/font/padding change |
| Comparing `font.family == Family::SansSerif` to detect non-Friz fonts | `Font::with_name()` uses `Family::Name(...)`, not `SansSerif` |
| Default `markdown::Style::from(&theme)` for link color | Ignores ThemeColors.link, uses theme primary instead |
| `vscroll()` with `.spacing(8)` in tables | Scrollbar reserves 18px, shifts columns left when it appears |
| `height(Shrink)` on clear-button overlay container in stack | Container sticks to top-left; `align_y(Center)` has no effect |
| External clear button in a `row![]` next to text input | Button renders outside the input field, not inline |
| Fixed `height(580)` on dialog row | Doesn't adapt to window size; looks small on tall monitors |

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
| `iced::widget::opaque()` wrapping the overlay stack | Complete event sink, blocks all layers below |
| `markdown::Content::parse()` + `.images()` for image URL collection | Guaranteed URL match with renderer |
| `text("ⓘ").size(17).line_height(1.0)` for Unicode icon tabs | Matches SVG icon button height exactly |
| Storing `Vec<markdown::Item>` in app state (dialog, release notes) | Avoids parse-in-view lifetime issues |
| `settings_card` (Shrink) + `settings_card_fill` (Fill) pattern for grid rows | Taller card anchors row height; shorter card stretches to match |
| `vscroll_overlay()` — `Scrollbar::new()` without `.spacing()` | Scrollbar floats over content, columns stay aligned |
| `height(Length::Fill)` on overlay container inside `stack![]` | Enables `align_y(Center)` to work correctly |
| `let el: Element<Message> = if cond { a.into() } else { Space::new().into() }` | Typed variable resolves `Space` type ambiguity in `container()` |
| `centered_dialog.padding(40)` + dialog_box `width/height(Fill)` | Dialog fills ~90% of window, compact dialogs still center correctly |
| `md_style.link_color = colors.link` before markdown settings | All links use consistent app link color |
| Loading both `NotoSans-Regular.ttf` and `NotoSans-Bold.ttf` | Iced selects bold automatically for `Weight::Bold` requests |
