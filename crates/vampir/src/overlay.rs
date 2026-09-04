//! Things that float above the view: tooltips and a command palette.

use std::rc::Rc;

use gpui::{
    AnyElement, AnyView, App, Context, ElementId, Entity, FontWeight, MouseButton, MouseDownEvent,
    Render, SharedString, Window, deferred, div, prelude::*, px,
};

use crate::controls::search_field;
use crate::lighting;
use crate::palette::Palette;
use crate::state::ControlHost;
use crate::text_input::TextInput;

// ---- Tooltip ----------------------------------------------------------------

/// A tooltip's content: one line, optionally with an accelerator after it.
///
/// GPUI already owns the hard part, which is deciding when a tooltip should
/// appear and where it fits. This is only what it looks like, handed to
/// `.tooltip(..)` on any interactive element.
pub struct Tooltip {
    text: SharedString,
    shortcut: Option<SharedString>,
    palette: Palette,
}

impl Tooltip {
    /// Builds the callback `.tooltip(..)` wants.
    ///
    /// ```ignore
    /// controls::icon_button("undo", glyph, 26.0, false, true, &palette, cx, ..)
    ///     .tooltip(Tooltip::text("Undo", palette))
    /// ```
    pub fn text(
        text: impl Into<SharedString>,
        palette: Palette,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let text = text.into();
        move |_window, cx| {
            let text = text.clone();
            cx.new(move |_cx| Tooltip {
                text,
                shortcut: None,
                palette,
            })
            .into()
        }
    }

    /// The same, with an accelerator shown after the label. Purely a label:
    /// binding the key is the host's business.
    pub fn with_shortcut(
        text: impl Into<SharedString>,
        shortcut: impl Into<SharedString>,
        palette: Palette,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let text = text.into();
        let shortcut = shortcut.into();
        move |_window, cx| {
            let (text, shortcut) = (text.clone(), shortcut.clone());
            cx.new(move |_cx| Tooltip {
                text,
                shortcut: Some(shortcut),
                palette,
            })
            .into()
        }
    }
}

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette;
        let dark = palette.is_dark;
        let fill = if dark {
            palette.soft_fill
        } else {
            palette.field_surface
        };
        div()
            .px(px(8.0))
            .py(px(4.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .rounded(px(5.0))
            .bg(fill)
            .border_1()
            .border_color(lighting::rim(fill, dark))
            .shadow(lighting::panel(dark))
            .text_size(px(11.5))
            .text_color(palette.text_primary)
            .whitespace_nowrap()
            .child(self.text.clone())
            .children(
                self.shortcut
                    .clone()
                    .map(|shortcut| div().text_color(palette.text_secondary).child(shortcut)),
            )
    }
}

// ---- Command palette --------------------------------------------------------

/// One entry in a [`command_palette`].
#[derive(Clone, Debug)]
pub struct Command {
    /// Comes back to the activation callback.
    pub id: SharedString,
    pub label: SharedString,
    /// Where the command lives, shown quietly before the label.
    pub group: Option<SharedString>,
    pub shortcut: Option<SharedString>,
}

impl Command {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            group: None,
            shortcut: None,
        }
    }

    pub fn group(mut self, group: impl Into<SharedString>) -> Self {
        self.group = Some(group.into());
        self
    }

    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }
}

/// How well a query matches a candidate, or `None` if it does not.
///
/// Subsequence matching, the way every fuzzy finder works: the query's
/// characters must appear in order, but not together. The score rewards
/// matches that start a word and matches that run on from the last one, so
/// "opf" ranks "Open File" above "Optional Prefix".
pub fn fuzzy_score(query: &str, candidate: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let candidate_lower = candidate.to_lowercase();
    let mut haystack = candidate_lower.char_indices().peekable();
    let mut score = 0i32;
    let mut last_index: Option<usize> = None;

    for needle in query.to_lowercase().chars() {
        if needle.is_whitespace() {
            continue;
        }
        loop {
            let (index, ch) = haystack.next()?;
            if ch != needle {
                continue;
            }
            // Starting a word is the strongest signal that this is the
            // match the person meant.
            let starts_word = index == 0
                || candidate_lower[..index]
                    .chars()
                    .next_back()
                    .is_some_and(|previous| previous == ' ' || previous == '-' || previous == '_');
            score += if starts_word { 12 } else { 2 };
            if last_index == Some(index.saturating_sub(1)) {
                score += 6;
            }
            last_index = Some(index);
            break;
        }
    }
    // Among equally good matches, the shortest candidate is the one that
    // most nearly *is* the query.
    Some(score - (candidate.len() as i32 / 8))
}

/// Commands matching a query, best first. Ties keep their original order,
/// so a host's own ranking survives where the score cannot separate two.
pub fn fuzzy_filter(query: &str, commands: &[Command]) -> Vec<Command> {
    let mut scored: Vec<(i32, usize, Command)> = commands
        .iter()
        .enumerate()
        .filter_map(|(index, command)| {
            let haystack = match &command.group {
                Some(group) => format!("{group} {}", command.label),
                None => command.label.to_string(),
            };
            fuzzy_score(query, &haystack).map(|score| (score, index, command.clone()))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, _, command)| command).collect()
}

/// Centred overlay with a filter field and a list of matching commands.
///
/// The host owns the query input, the filtered list and the highlighted
/// index, because all three are also what the up and down keys move and
/// what Enter commits, and those bindings belong with the host's other
/// keys. [`fuzzy_filter`] does the ranking.
#[allow(clippy::too_many_arguments)]
pub fn command_palette<V: ControlHost>(
    id: &'static str,
    query: &Entity<TextInput>,
    matches: &[Command],
    highlighted: usize,
    palette: Palette,
    window: &Window,
    cx: &mut Context<V>,
    on_activate: impl Fn(&mut V, SharedString, &mut Window, &mut Context<V>) + 'static,
    on_dismiss: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let dark = palette.is_dark;
    let on_activate = Rc::new(on_activate);
    let fill = if dark {
        palette.soft_fill
    } else {
        palette.field_surface
    };

    let mut rows: Vec<AnyElement> = Vec::with_capacity(matches.len());
    for (index, command) in matches.iter().enumerate() {
        let cx: &mut Context<V> = &mut *cx;
        let on_activate = on_activate.clone();
        let command_id = command.id.clone();
        let active = index == highlighted;
        rows.push(
            div()
                .id(ElementId::NamedInteger(
                    format!("{id}-command").into(),
                    index as u64,
                ))
                .h(px(30.0))
                .flex_none()
                .w_full()
                .px(px(9.0))
                .flex()
                .items_center()
                .gap(px(8.0))
                .rounded(px(5.0))
                .cursor_pointer()
                .text_size(px(12.5))
                .text_color(if active {
                    palette.control_label
                } else {
                    palette.text_primary
                })
                .when(active, |el| {
                    el.bg(lighting::lit(palette.control_fill, 0.08))
                })
                .when(!active, |el| {
                    el.hover(move |style| style.bg(palette.row_hover))
                })
                .on_click(cx.listener(move |this, _event, window, cx| {
                    on_activate(this, command_id.clone(), window, cx);
                    cx.notify();
                }))
                .children(command.group.clone().map(|group| {
                    div()
                        .flex_none()
                        .text_color(palette.text_secondary)
                        .child(group)
                }))
                .child(
                    div()
                        .flex_1()
                        .overflow_hidden()
                        .child(command.label.clone()),
                )
                .children(command.shortcut.clone().map(|shortcut| {
                    div()
                        .flex_none()
                        .text_size(px(11.5))
                        .text_color(palette.text_secondary)
                        .child(shortcut)
                }))
                .into_any_element(),
        );
    }
    let empty = rows.is_empty();

    deferred(
        div()
            .id(ElementId::Name(format!("{id}-scrim").into()))
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .items_center()
            .bg(palette.scrim())
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _event: &MouseDownEvent, window, cx| {
                    on_dismiss(this, window, cx);
                    cx.notify();
                }),
            )
            .child(
                div()
                    // A little above centre: the list grows downward, and a
                    // palette pinned to the middle ends up low on the screen
                    // as soon as it has results.
                    .mt(gpui::relative(0.16))
                    .w(px(520.0))
                    .max_w(gpui::relative(0.9))
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .p(px(8.0))
                    .rounded(px(10.0))
                    .bg(fill)
                    .border_1()
                    .border_color(lighting::rim(fill, dark))
                    .shadow(lighting::panel(dark))
                    .occlude()
                    .child(search_field(id, query, palette, window, cx))
                    .child(
                        div()
                            .id(ElementId::Name(format!("{id}-list").into()))
                            .max_h(px(340.0))
                            .flex()
                            .flex_col()
                            .gap(px(1.0))
                            .overflow_y_scroll()
                            .children(rows)
                            .when(empty, |el| {
                                el.child(
                                    div()
                                        .h(px(30.0))
                                        .px(px(9.0))
                                        .flex()
                                        .items_center()
                                        .text_size(px(12.5))
                                        .font_weight(FontWeight::NORMAL)
                                        .text_color(palette.text_secondary)
                                        .child("No matching commands"),
                                )
                            }),
                    ),
            ),
    )
    .with_priority(180)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_starts_outrank_scattered_letters() {
        let open_file = fuzzy_score("opf", "Open File").expect("matches");
        let scattered = fuzzy_score("opf", "Optional Prefix").expect("matches");
        assert!(
            open_file > scattered,
            "expected Open File ({open_file}) above Optional Prefix ({scattered})"
        );
    }

    #[test]
    fn a_query_that_is_not_a_subsequence_does_not_match() {
        assert!(fuzzy_score("zzz", "Open File").is_none());
        assert!(fuzzy_score("elif", "Open File").is_none());
    }

    #[test]
    fn an_empty_query_keeps_everything_in_its_original_order() {
        let commands = vec![Command::new("b", "Second"), Command::new("a", "First")];
        let filtered = fuzzy_filter("", &commands);
        let ids: Vec<&str> = filtered.iter().map(|command| command.id.as_ref()).collect();
        assert_eq!(ids, vec!["b", "a"]);
    }

    #[test]
    fn the_group_is_searched_along_with_the_label() {
        let commands = vec![Command::new("save", "Save").group("File")];
        assert_eq!(fuzzy_filter("file sa", &commands).len(), 1);
    }
}
