//! Shortcut recorder: a field that captures the next key chord.
//!
//! It records what was pressed and hands back a description. Binding the
//! result is the host's business, because only the host knows what its
//! actions are and gpui wants a keymap, not a widget, to own them.

use gpui::{
    Context, ElementId, FocusHandle, KeyDownEvent, MouseButton, MouseDownEvent, SharedString,
    Window, div, prelude::*, px,
};

use crate::controls::{CONTROL_HEIGHT, CONTROL_RADIUS};
use crate::lighting;
use crate::palette::Palette;
use crate::state::{ComboId, ControlHost};

/// A captured key chord, in gpui's own keystroke notation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chord {
    /// What to give a `KeyBinding`, for instance `cmd-shift-p`.
    pub keystroke: SharedString,
    /// The same chord for a person to read, for instance `⌘⇧P`.
    pub display: SharedString,
}

/// Turns a key event into a chord, or `None` if it is not one worth
/// recording.
///
/// A bare modifier is skipped: while someone holds Command on the way to
/// Command-S, every one of those presses arrives here, and recording the
/// first would end the capture before they got to the letter.
pub fn chord_from(event: &KeyDownEvent) -> Option<Chord> {
    let keystroke = &event.keystroke;
    let key = keystroke.key.as_str();
    if key.is_empty() || matches!(key, "cmd" | "ctrl" | "alt" | "shift" | "fn" | "function") {
        return None;
    }
    let modifiers = &keystroke.modifiers;

    let mut parts: Vec<&str> = Vec::new();
    if modifiers.control {
        parts.push("ctrl");
    }
    if modifiers.alt {
        parts.push("alt");
    }
    if modifiers.shift {
        parts.push("shift");
    }
    if modifiers.platform {
        parts.push("cmd");
    }
    parts.push(key);

    let mut display = String::new();
    if modifiers.control {
        display.push('\u{2303}');
    }
    if modifiers.alt {
        display.push('\u{2325}');
    }
    if modifiers.shift {
        display.push('\u{21e7}');
    }
    if modifiers.platform {
        display.push('\u{2318}');
    }
    display.push_str(&pretty_key(key));

    Some(Chord {
        keystroke: parts.join("-").into(),
        display: display.into(),
    })
}

/// A key's name as it appears on a key cap.
fn pretty_key(key: &str) -> String {
    match key {
        "enter" => "\u{21a9}".into(),
        "tab" => "\u{21e5}".into(),
        "space" => "Space".into(),
        "backspace" => "\u{232b}".into(),
        "delete" => "\u{2326}".into(),
        "escape" => "Esc".into(),
        "up" => "\u{2191}".into(),
        "down" => "\u{2193}".into(),
        "left" => "\u{2190}".into(),
        "right" => "\u{2192}".into(),
        other if other.chars().count() == 1 => other.to_uppercase(),
        other => {
            let mut chars = other.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        }
    }
}

/// A field that shows a shortcut and, when clicked, captures the next chord.
///
/// `focus` must be a handle the host keeps for this recorder: capturing keys
/// means taking focus, and focus has to outlive a frame. While recording,
/// Escape cancels and every other chord is recorded and ends the capture.
#[allow(clippy::too_many_arguments)]
pub fn shortcut_recorder<V: ControlHost>(
    id: ComboId,
    current: Option<&Chord>,
    focus: &FocusHandle,
    palette: Palette,
    view: &V,
    cx: &mut Context<V>,
    on_record: impl Fn(&mut V, Chord, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let recording = view.control_state().recording == Some(id);
    let label: SharedString = match (recording, current) {
        (true, _) => "Press a shortcut\u{2026}".into(),
        (false, Some(chord)) => chord.display.clone(),
        (false, None) => "Not set".into(),
    };
    let focus_for_click = focus.clone();

    div()
        .id(ElementId::Name(format!("{id}-recorder").into()))
        .track_focus(focus)
        .key_context("ShortcutRecorder")
        .h(px(CONTROL_HEIGHT))
        .w_full()
        .px(px(9.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(CONTROL_RADIUS))
        .bg(palette.field_surface)
        .border_1()
        .border_color(if recording {
            palette.accent
        } else {
            palette.field_border
        })
        .shadow({
            let mut shadows = lighting::recessed(palette.is_dark);
            if recording {
                shadows.push(lighting::glow(palette.accent, 0.4, 4.0));
            }
            shadows
        })
        .cursor_pointer()
        .text_size(px(12.5))
        .text_color(if current.is_some() || recording {
            palette.text_primary
        } else {
            palette.text_secondary
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _event: &MouseDownEvent, window, cx| {
                this.control_state_mut().recording = Some(id);
                window.focus(&focus_for_click, cx);
                cx.notify();
            }),
        )
        .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
            if this.control_state().recording != Some(id) {
                return;
            }
            // Escape leaves the current shortcut alone, which is the only
            // way out for someone who opened this by accident.
            if event.keystroke.key == "escape" {
                this.control_state_mut().recording = None;
                cx.notify();
                return;
            }
            if let Some(chord) = chord_from(event) {
                this.control_state_mut().recording = None;
                on_record(this, chord, window, cx);
                cx.notify();
            }
        }))
        .child(label)
}
