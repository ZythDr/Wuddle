# Iced 0.14 — Wuddle Development Notes

Discoveries made while porting the Tauri/Svelte Wuddle frontend to Iced 0.14.

---

## Markdown Widget

### Basic usage

```rust
// Parse once, store in state (Item is Clone + owns all its data)
let items: Vec<iced::widget::markdown::Item> = iced::widget::markdown::parse(&readme_text).collect();

// Render in view — returns Element<'a, String> where String is a clicked URL
iced::widget::markdown::view(&items, &theme)
    .map(Message::OpenUrl)
```

`markdown::view` takes `(items: impl IntoIterator<Item = &'a Item>, settings: impl Into<Settings>)`.
In iced 0.14 the second argument is `Settings`, not the theme directly — but `Settings` implements
`From<&Theme>` so `&theme` is accepted.

### Item enum variants (relevant ones)

```rust
iced::widget::markdown::Item::Image { url: String, .. }
```

The `url` field is the raw string as it appears in the markdown (may be relative).

### Custom Viewer for images

The default viewer renders image alt text in a code-block container. To display actual images,
implement the `Viewer` trait:

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
        let bytes = if let Some(b) = self.cache.get(url.as_str()) {
            Some(b)
        } else {
            let abs = resolve_image_url(url, self.raw_base_url);
            self.cache.get(abs.as_str())
        };
        if let Some(bytes) = bytes {
            iced::widget::image(iced::widget::image::Handle::from_bytes(bytes.clone()))
                .width(Length::Fill)
                .into()
        } else {
            Space::new().height(0).into()
        }
    }
}
```

Then use `markdown::view_with` instead of `markdown::view`:

```rust
let viewer = ImageViewer { cache: &preview.image_cache, raw_base_url: &preview.raw_base_url };
let settings = iced::widget::markdown::Settings::with_text_size(13, Style::from(&theme));
iced::widget::markdown::view_with(&items, settings, &viewer)
```

**Key lifetime rule:** `ImageViewer<'a>` must satisfy `Self: 'a` (i.e., all references inside must
live at least as long as `'a`). The cache reference must come from app state that outlives the
view call — typically borrowed from `self.add_repo_preview`.

### Storing parsed items in state

**Problem:** `markdown::view` borrows the items and the returned `Element` has the same lifetime.
If you parse inside the view function, the `Vec<Item>` is a local and the element outlives it.

**Solution:** Store the parsed `Vec<Item>` in app state (e.g., inside `RepoPreviewInfo`). Since
`Item` is `Clone` and owns all its string data, this works fine.

```rust
pub struct RepoPreviewInfo {
    pub readme_items: Vec<iced::widget::markdown::Item>,
    // ...
}
// Parse once when fetching:
let readme_items: Vec<iced::widget::markdown::Item> = markdown::parse(&readme_text).collect();
```

### GIF / animated images

Iced 0.14 does **not** support animated GIFs. The `image::Handle::from_bytes()` will display
the first frame only. There is no built-in animation support; a custom canvas-based approach
would be required for GIF animation (not implemented).

### Settings / text sizing

```rust
// Create settings with a specific base text size and style derived from the current theme
let settings = iced::widget::markdown::Settings::with_text_size(13, Style::from(&theme));
// Heading sizes are derived automatically (h1 = 2× base, h2 = 1.75×, etc.)
```

---

## Image Loading

### `image::Handle::from_bytes`

```rust
// Load PNG/JPEG from a Vec<u8>
let handle = iced::widget::image::Handle::from_bytes(bytes.clone());
iced::widget::image(handle).width(Length::Fill)
```

`from_bytes` accepts any `Into<Bytes>` — `Vec<u8>` works directly.

### Fetching remote images

Use `reqwest` with tokio timeouts and size limits:

```rust
let result = tokio::time::timeout(
    Duration::from_secs(3),
    client.get(&url).send(),
).await;
if let Ok(Ok(resp)) = result {
    if resp.status().is_success() {
        if let Ok(bytes) = resp.bytes().await {
            if bytes.len() <= 1_500_000 { // 1.5 MB limit
                cache.insert(url, bytes.to_vec());
            }
        }
    }
}
```

Recommended limits: max 8 images, 1.5 MB each, 6 MB total.

### Resolving relative image URLs

GitHub raw base: `https://raw.githubusercontent.com/{owner}/{repo}/HEAD/`

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

Store both the original URL and the resolved absolute URL in the cache so lookups work
regardless of which form the markdown uses.

---

## Keyboard Event Subscription (Escape to close dialogs)

```rust
fn subscription(&self) -> Subscription<Message> {
    let mut subs = Vec::new();
    // ...other subscriptions...

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

Gating on `self.dialog.is_some()` means the subscription is only active when needed, avoiding
unnecessary event processing.

---

## Dialog Pattern

### Dialog enum with mutable field state

Fields like URL input, mode selection, and advanced toggle are stored directly in the `Dialog`
enum variant rather than in the `App` struct. This keeps all dialog-related state together.

For fields that need to change without clearing a preview (e.g., URL input), store the preview
in `App` rather than in the `Dialog`:

```rust
// In App:
pub add_repo_preview: Option<RepoPreviewInfo>,  // lives outside dialog
pub add_repo_preview_loading: bool,

// URL input gets its own message to avoid triggering OpenDialog (which would reset the preview):
Message::SetAddRepoUrl(String)  // updates url in dialog + fires preview fetch
// vs
Message::OpenDialog(Dialog::AddRepo { url, mode, ..., advanced: !advanced })  // for toggle only
```

### Two-card floating side panel layout (matching Tauri)

When a preview is loaded, the dialog switches from a single card to **two separate cards**
side by side, matching the Tauri version's floating side panel approach:

```
+--sidebar card (260px)--+  +--main form card (fills)----+
| About                  |  | Title               [×]    |
| Atlas-TW               |  | Subtitle                   |
| Description text...    |  | URL input                  |
| ★ 16                   |  | ┌─README (scrollable)────┐ |
| ⑂ 1                    |  | │ Heading                 │ |
| Language         Lua   |  | │ Images render inline    │ |
| ──────────────────     |  | │ Text continues...       │ |
| Files                  |  | └─────────────────────────┘ |
| ▸ Images/              |  | [Advanced □] [Cancel] [Add] |
| ▸ Locales/             |  +-----------------------------+
|   Atlas-TW.toc         |
|   README.md            |
+------------------------+
```

**Key implementation details:**
- `view()` detects `has_two_cards` (AddRepo + preview loaded) and wraps the dialog in a
  transparent outer container with `max_width(1060)` — no padding or style
- `view_dialog()` returns a `row![sidebar_card, form_card]` where **each card** has its own
  `dialog_style` (border, background gradient, shadow, rounded corners)
- The sidebar card is 260px fixed width with its own padding
- The form card fills remaining space
- Both cards sit inside a row with `height(560)` and `spacing(8)` gap
- Without a preview, falls back to single-card layout (700px, dialog_style on outer container)

**Why two cards instead of one?** The Tauri version uses `display: flex` with two siblings
(`.repo-side-panel` + `.dialog-inner`) each having their own `border`, `border-radius`,
`background-image`, and `box-shadow`. A single card with an internal sidebar can't replicate
the floating panel look.

### Click-through prevention

**Critical finding:** iced 0.14's `stack!` dispatches events to ALL layers simultaneously
via `fold(Ignored, Status::merge)`. A `mouse_area` in one layer cannot prevent lower layers
from also processing the same event. This means widgets in `main_content` (buttons, etc.)
can still fire even when a dialog overlay is displayed on top.

**Solution:** Use `iced::widget::opaque()` to wrap the entire overlay, which absorbs ALL
mouse events and prevents them from reaching any lower layer:

```rust
// Wrap the scrim + dialog stack in opaque() so main_content is completely blocked
let overlay = iced::widget::opaque(
    stack![scrim, centered_dialog]
        .width(Length::Fill)
        .height(Length::Fill),
);
stack![main_content, overlay]
```

`iced::widget::opaque` is available via `pub use helpers::*` in `iced_widget` which is
re-exported through `iced::widget`. It returns `event::Status::Captured` for every event
regardless of what inner widgets return, effectively making the overlay a complete event sink.

**Also wrap dialog in mouse_area** so click on the dialog itself doesn't fire the scrim:

```rust
let dialog_blocker = iced::widget::mouse_area(dialog_box)
    .on_press(Message::CloseMenu);  // harmless — just clears context menus

let centered_dialog = container(dialog_blocker)
    .center_x(Length::Fill)
    .center_y(Length::Fill);
```

**Do NOT** rely only on `mouse_area` without `opaque` — `mouse_area` alone cannot stop
events from reaching lower layers in a `stack!`.

### Font colors: text vs text_soft vs muted

The Tauri version uses three text color tiers. The iced theme now matches:

| Tier | CSS Variable | Usage | ThemeColors field |
|------|-------------|-------|-------------------|
| Bright | `--text` | File names, headings, interactive text | `colors.text` |
| Soft | `--text-soft` | Body text, descriptions, README, stats | `colors.text_soft` |
| Dim | `--muted` | Labels ("About", "Files"), hints, placeholders | `colors.muted` |

Use `text_soft` for anything that should be readable but not prominent (descriptions,
README body, sidebar stats). Use `muted` only for labels and structural text.

---

## Pick List with String items

`iced::widget::pick_list` requires `T: ToString + PartialEq + Clone + 'static`. Using
`Vec<String>` works directly. Pass `Some(selected.clone())` as the current selection.

```rust
let modes = vec!["auto".to_string(), "addon".to_string(), "dll".to_string()];
iced::widget::pick_list(modes, Some(current_mode.clone()), |new_mode: String| {
    Message::OpenDialog(Dialog::AddRepo { mode: new_mode, ..rest })
})
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

The font name must match the font's internal name metadata, not the filename. Use a font
inspection tool if `Font::with_name(...)` doesn't render (the name might differ from the filename).

**Note:** `default_font` is set once at startup. Changing it at runtime requires a restart;
there is no hot-reload. Log a message telling the user to restart when they toggle the font setting.

---

## Auto-check Timer

```rust
fn subscription(&self) -> Subscription<Message> {
    if self.opt_auto_check {
        let mins = self.auto_check_minutes.max(1) as u64;
        iced::time::every(Duration::from_secs(mins * 60)).map(|_| Message::AutoCheckTick)
    } else {
        Subscription::none()
    }
}
```

---

## Spinner Animation

A canvas-based spinner that uses a subscription:

```rust
// In subscription():
if self.is_busy() {
    subs.push(
        iced::time::every(Duration::from_millis(80))
            .map(|_| Message::SpinnerTick),
    );
}

// In update():
Message::SpinnerTick => { self.spinner_tick = (self.spinner_tick + 1) % 36; }

// In view (always reserves space so layout doesn't shift):
if self.is_busy() {
    canvas(SpinnerCanvas { tick, color }).width(26).height(26).into()
} else {
    Space::new().width(26).height(26).into()  // same size, invisible
}
```

---

## forge / GitHub API patterns

### Collect image URLs from parsed markdown

```rust
fn collect_image_urls(items: &[markdown::Item]) -> Vec<String> {
    items.iter().filter_map(|item| {
        if let markdown::Item::Image { url, .. } = item {
            Some(url.clone())
        } else {
            None
        }
    }).collect()
}
```

### GitHub API endpoints used

- Repo info: `GET https://api.github.com/repos/{owner}/{repo}`
  - Headers: `Accept: application/vnd.github+json`
  - Auth: `Authorization: Bearer {token}` (optional but needed for rate limits)
- README (raw): `GET https://api.github.com/repos/{owner}/{repo}/readme`
  - Headers: `Accept: application/vnd.github.raw+json`
- File tree: `GET https://api.github.com/repos/{owner}/{repo}/contents/`

### GitLab API endpoints

- Repo info: `GET https://gitlab.com/api/v4/projects/{owner%2Frepo}`
- README: `GET https://gitlab.com/{owner}/{repo}/raw/HEAD/README.md`
- File tree: `GET https://gitlab.com/api/v4/projects/{owner%2Frepo}/repository/tree?per_page=50`

### Gitea/Forgejo/Codeberg API endpoints

- Repo info: `GET {scheme}://{host}/api/v1/repos/{owner}/{repo}`
- README: `GET {scheme}://{host}/{owner}/{repo}/raw/branch/master/README.md`
- File tree: `GET {scheme}://{host}/api/v1/repos/{owner}/{repo}/contents/`
