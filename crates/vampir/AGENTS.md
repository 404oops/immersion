# Vampir — agent guide

Vampir is a widget toolkit for [GPUI](https://www.gpui.rs). It is a plain
library crate with no application inside it: `gpui` and
`unicode-segmentation` are the only dependencies, and it builds anywhere
GPUI does.

Read this before you use it and before you change it. The first half is how
to get the most out of what is here; the second is the design it has to keep
if it is going to stay coherent.

---

## 1. The one idea

**A control is a function of the data it is handed.** It takes a value, a
`Palette`, and a `Context<V>`, and returns an element. It owns nothing.

What cannot be a pure function is small, always the same shape, and lives in
one place: `ControlState`. That is which pop-up is open and how far into its
fade, when each animated control last changed, and whatever is being
dragged. Nothing else.

This is why every control is generic over the host view rather than tied to
one. If you find yourself wanting to store a widget in the host, or give a
widget a handle to the host, you have taken a wrong turn.

---

## 2. Setting a host up

Four things, once:

```rust
use gpui::{Context, Point, Window, prelude::*};
use vampir::{ControlHost, ControlState, Palette};

struct Editor {
    controls: ControlState,
    palette: Palette,
    // ... the rest of the app
}

impl ControlHost for Editor {
    fn control_state(&self) -> &ControlState { &self.controls }
    fn control_state_mut(&mut self) -> &mut ControlState { &mut self.controls }

    // Only if you use sliders, splits or the colour pad.
    fn track_dragged(&mut self, id: vampir::ComboId, at: Point<f32>, cx: &mut Context<Self>) {
        match id {
            "zoom" => self.set_zoom(at.x, cx),
            "sidebar-split" => self.sidebar_fraction = at.x,
            _ => {}
        }
    }

    // Only if you use the tab bar.
    fn tabs_reordered(&mut self, _bar: vampir::ComboId, from: usize, to: usize, cx: &mut Context<Self>) {
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        cx.notify();
    }

    // Only if you track drags of your own at the root. Scrollbar tracks
    // block the mouse, so they forward through these.
    fn forwarded_mouse_move(&mut self, event: &gpui::MouseMoveEvent, _w: &mut Window, cx: &mut Context<Self>) {
        self.root_mouse_move(event, cx);
    }
    fn forwarded_mouse_up(&mut self, _w: &mut Window, cx: &mut Context<Self>) {
        self.root_mouse_up(cx);
    }
}
```

### Pump the drags

Every drag outlives the control that started it: the pointer leaves a 5px
slider track immediately and every move after that arrives somewhere else.
So the host pumps them from wherever it tracks the pointer at the root:

```rust
fn root_mouse_move(&mut self, event: &gpui::MouseMoveEvent, cx: &mut Context<Self>) {
    // A release outside the window never arrives. A move with no button
    // held is that release, so end everything exactly as it would have.
    if !event.dragging() {
        if self.controls.dragging_anything() { self.root_mouse_up(cx); }
        return;
    }
    if vampir::continue_drags(self, event.position, cx) {
        cx.notify();
    }
}

fn root_mouse_up(&mut self, cx: &mut Context<Self>) {
    vampir::end_drags(self, cx);   // applies a finished tab reorder too
    cx.notify();
}
```

Attach that mouse-move handler to the root **and to every `.occlude()`d
surface**. An occluding panel swallows the move stream a drag underneath it
depends on.

### Ask for frames while things animate

Fades and slides are `Instant`-driven, so they finish on their own and stop
asking for frames. Fold `ControlState::animating()` into whatever decides:

```rust
if self.controls.animating() || self.my_own_transitions_running() {
    window.request_animation_frame();
}
```

Do not drive hover from an `Instant`. Scrolling a list re-fires hover on
every row and will pin the display link at 60Hz. Use GPUI's `.hover()`.

### Ids

`ComboId` is `&'static str`. It is both the element id and the widget's
identity in `ControlState`, so **two widgets of the same kind in one view
must not share one**. There is no enum to keep in step, which is the point.

---

## 3. What is in here

### Primitives — `vampir::controls`

| | |
|---|---|
| `caption` | Quiet section label. |
| `button` | The lit button. `ButtonVariant::{Soft, Primary, Danger}`. |
| `icon_button` | Square, takes any element as its icon. `active` gives it the pressed-in look of a toggle. |
| `switch` | Toggle. Animates on real toggles only. |
| `checkbox` | With an optional label; the whole row is the hit target. |
| `radio_group` | Vertical exclusive choices, each able to carry a `detail` line. |
| `segmented` | Recessed track, one raised segment. Two or three short options. |
| `chip` / `chip_group` | Buttons sized to their own labels, in a wrapping row. For a choice with more options than a segmented control can carry but all worth showing: twenty lighting effects, tags, a filter bar. |
| `spinbox` | Steppers around an editable value field. |
| `text_field` / `text_area` | Recessed wells around a `TextInput`. |
| `search_field` | Field with a magnifier and a clear button. |
| `combo` | Pop-up menu button and its list. `ComboDirection::Up` near a window's bottom. |
| `slider` | `SliderTrack::Continuous` or `Stepped { stops }`, which draws ticks and snaps. |
| `progress_bar` | Determinate. |
| `spinner` | Indeterminate. Only animates while the host is asking for frames. |
| `badge` | `BadgeTone::{Neutral, Accent, Danger}`. |
| `separator` | Hairline rule, horizontal or vertical. |
| `scrollbar` | Overlay bar. Put it in a `.relative()` wrapper around the scroll container; it renders nothing while the content fits. |

### Containers — `vampir::containers`

`tab_bar` (drag to reorder, optional close buttons), `split_handle` +
`split_area`, `collapsible`, `dialog` + `DialogButton`.

`split_area` is the invisible probe that turns a pointer position into a
fraction. Put it inside the element the two panes share; the divider will
not work without it.

### Menus — `vampir::menu`

`context_menu` renders the open menu; `MenuItem::action(..).shortcut(..)
.checked(..).danger().disabled()` builds the rows. Open it from wherever the
press happened:

```rust
.on_mouse_down(MouseButton::Right, cx.listener(|this, event: &MouseDownEvent, _w, cx| {
    this.control_state_mut().open_menu("row", event.position, row_id.to_string());
    cx.notify();
}))
```

and render it once, near the root:

```rust
.children(vampir::context_menu("row", &self.menu_items(), self.palette, self, cx,
    |host, action, window, cx| host.run_menu_action(action, window, cx)))
```

The `target` string is yours and comes back untouched through
`ControlState::menu_target()`; build the item list from it each frame, so the
menu reflects what was actually clicked. `menu_button` opens the same menu
hanging beneath a button instead.

### Overlays — `vampir::overlay`

`Tooltip::text` / `Tooltip::with_shortcut` plug into GPUI's own
`.tooltip(..)`, which already owns the hover delay and the placement.
`command_palette` plus `fuzzy_filter` / `fuzzy_score`: the host owns the
query input, the filtered list and the highlighted index, because those are
also what the arrow keys move and what Enter commits.

### Data — `vampir::data`

`tree_row` renders one row of an already flattened tree, so it drops
straight into a `uniform_list`. `table_header` with `Column` and
`SortDirection`.

### Colour — `vampir::swatch`

`hue_slider`, `color_pad` (chroma across, lightness up), `swatch_grid`,
`hue_wheel`. Everything is `Oklch`.

### Shortcuts — `vampir::shortcut`

`shortcut_recorder` captures the next chord and hands back a `Chord` with
both a `keystroke` for `KeyBinding` and a `display` for a person. Binding it
is the host's business.

### Text — `vampir::text_input`

`TextInput` is a full input: selection, IME composition, undo, clipboard,
multi-line wrapping. Rich text is a `Highlighter`, a closure from content to
`Span`s, re-run on every layout:

```rust
input.update(cx, |input, cx| {
    input.set_highlighter(|text| {
        find_mentions(text).map(|range| Span::new(range).color(accent).bold()).collect()
    }, cx);
});
```

Stale ranges are dropped rather than panicked on, because a highlighter
usually runs against text that has since been edited.

### Colour and lighting — `vampir::color`, `vampir::lighting`

`Palette::from_hue(hue, dark)` derives the whole set from one hue. A host
with a richer theme of its own writes the twenty-odd fields directly; see
`Theme::palette` in the Immersion app for what that looks like.

---

## 4. Design conventions

These are not preferences. They came out of a long argument with a real
user, and the flat, generic versions were rejected twice before this landed.

### Light comes from above

Everything reads as a physical surface lit from the top. That is the entire
visual thesis, and `lighting` is the whole vocabulary:

- `lit(fill, lift)` — the vertical gradient. Every filled control gets one.
- `raised(dark)` — inner top highlight, outer drop shadow. Buttons, pop-up
  buttons, active segments, tabs.
- `recessed(dark)` — inner top shadow. Fields, tracks, wells, anything you
  can type or drag into.
- `panel(dark)` — lit top edge and a wide soft shadow. Floating things.
- `rim(fill, dark)` — a control's border, derived from its own fill.
- `glow(color, alpha, radius)` — focus rings and current-item halos.
- `shade(color, amount)` — toward white or black.
- `faded(shadows, t)` — the same shadows at `t` strength, for fades.

**Raised means you press it. Recessed means you put something in it.** If a
new control is neither, work out which it is before drawing it.

Never hard-code a shadow or a gradient. If a recipe does not exist for what
you need, add it to `lighting` where the next control can find it.

### Never do these

Each was tried and rejected by name:

- A coloured accent bar down the side of a selected row. Use a lit accent
  fill across the whole row.
- Underlined text tabs. Tabs are raised surfaces.
- Tiny uppercase micro-captions. Use `caption`, sentence case.
- Monospace for values and labels. Proportional type everywhere; mono is for
  identifiers, hashes and paths only.
- Dots, pills and chips as status indicators. `badge` exists for counts and
  states; a status is usually better said in words.
- Uniform rounded cards nested inside uniform rounded cards.
- Hairline dividers between every row of a list.
- Flat, shadowless surfaces.

### Metrics

`CONTROL_HEIGHT` (30) and `CONTROL_RADIUS` (6) are shared so a row of mixed
controls lines up with no per-control tuning. Use them. `CHIP_HEIGHT` (26) is
the one deliberate exception: a wrapping field of thirty full-height controls
reads as a wall rather than as a set of choices. Nested radii step
down by 1 or 2, never up. Text is 12.5px for controls, 11.5px for secondary
text, 14px for a dialog title. Do not introduce a fourth size without a
reason you can write down.

### Colour

Only the roles in `Palette`. If a control needs a colour that is not there,
either it is one of the existing roles under another name, or the palette is
genuinely missing a role — and adding one means adding it to `from_hue` for
every hue and both schemes, and checking the result.

Both schemes always. A light-mode-only or dark-mode-only path is a bug; the
`is_dark` flag exists so one code path covers both. `from_hue` has a test
that walks every 15 degrees through both schemes and asserts the results
stay in gamut; extend it when you add a role.

### Interaction

- **Select on release, not on press.** Pressing is also how a drag starts,
  and a drag must not leave a trail of selections behind it. `tab_bar` does
  this; anything draggable must.
- **A press that never moved is a click.** That is the `moved` flag on every
  drag.
- **Handle the release that never comes.** A pointer released outside the
  window sends nothing. A move with no button held is that release.
- **Blocking the mouse blocks hover too.** Anything with
  `.block_mouse_except_scroll()` or `.occlude()` must forward moves and
  releases, or drags underneath it stall.
- **Disable, do not remove.** A control at its limit greys out and stays put.
  Removing it shifts its neighbour under the pointer.
- **Full width is a decision, not a default.** `button` fills its slot, which
  is right for a dialog footer and wrong for a list of twenty choices. When
  the options are a set rather than a sequence of actions, use `chip`.
- **Deferred priorities:** context menus 200, command palette 180, dialogs
  150, pop-up lists 100. Keep new overlays inside that ordering.

### Code

- **Every control is a free function generic over `V: ControlHost`.** No
  structs with builders, no trait objects, no `RootView` anywhere.
- **Build children in a `for` loop, not in `.children(map(..))`,** whenever
  each child needs `cx` for a listener. A closure passed to `map` is `FnMut`
  and cannot hand `&mut Context` out twice. Reborrow per iteration with
  `let cx: &mut Context<V> = &mut *cx;`.
- **Take `Palette` by value.** It is `Copy`, and a returned `impl
  IntoElement` cannot borrow a temporary.
- **Comments say why, not what.** `.h(px(26.0))` needs no comment. The
  reason a marker expires on its own does.
- **Test the logic, not the pixels.** Fuzzy scoring, span sanitising, gamut
  clamping and geometry are all testable without a window; the drawing is
  not, so keep drawing thin and put the thinking somewhere a test can reach.

### Adding a component

1. Decide it is raised or recessed, and give it `CONTROL_HEIGHT` if it sits
   in a row with others.
2. Free function, generic over `V: ControlHost`, taking `Palette` by value.
3. If it needs state between frames, ask hard whether the host should own it
   instead. `collapsible` takes `expanded` as a parameter for exactly this
   reason: whether a section is open usually outlives the view.
4. If it drags, use the existing `TrackDrag` mechanism rather than a new
   field. Sliders, splits and the colour pad are all one gesture over
   different geometry.
5. Export it from `lib.rs`, add a row to the table above, and add a test for
   whatever part of it is not drawing.

---

## 5. Known divergences

The Immersion app still uses its own folder tabs rather than
`containers::tab_bar`, because they carry inline rename and per-tab context
behaviour the generic bar does not model. Either the bar grows to cover
them, or the app keeps its own. Do not half-migrate.
