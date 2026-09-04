//! The mutable bits the controls need between frames.
//!
//! Most of a control is a pure function of the data it is handed. What is
//! left over is small and always the same shape: which pop-up is open and
//! how far into its fade it is, when each animated control last changed, and
//! whatever is being dragged. A host keeps one [`ControlState`], implements
//! [`ControlHost`] for its view, and every control is generic over that
//! host, so nothing here knows the host's type.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use gpui::{Bounds, Context, ElementId, MouseMoveEvent, Pixels, Point, SharedString, Window};

pub use crate::scroll::{ScrollAxis, ScrollDrag, apply_scroll_drag};

/// A pop-up list fades and slides into place over this long, and back out
/// over the same.
pub const COMBO_REVEAL: Duration = Duration::from_millis(140);
/// A switch's knob and track take this long to cross over. Disclosure
/// chevrons and other small state changes use it too, so everything in a
/// view turns at one speed.
pub const SWITCH_SLIDE: Duration = Duration::from_millis(140);

/// Identifies one pop-up, menu, track or tab bar. An element id doubles as
/// its identity, so a host needs no enum of its own. Two of the same kind of
/// widget in one view must not share one.
pub type ComboId = &'static str;

/// Which way a drag reads its position out of a track.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrackAxis {
    Horizontal,
    Vertical,
    /// A two-dimensional pad, such as a colour field.
    Both,
}

/// A drag along some widget's track: a slider handle, a split divider, a
/// colour pad. All of them are the same gesture over different geometry, so
/// they share one mechanism.
#[derive(Clone, Debug)]
pub struct TrackDrag {
    pub id: ComboId,
    pub axis: TrackAxis,
    /// Evenly spaced stops to snap to, if the track has any.
    pub stops: Option<u32>,
}

/// A tab being dragged along its bar.
#[derive(Clone, Debug)]
pub struct TabDrag {
    pub bar: ComboId,
    /// Where the tab started.
    pub from: usize,
    /// Where it would land if released now.
    pub to: usize,
    /// Whether the pointer has actually moved. A press that never moves is a
    /// click on the tab, not a reorder.
    pub moved: bool,
}

/// A context menu that is open: which menu, where it was summoned, and what
/// it was summoned on.
#[derive(Clone, Debug)]
pub struct OpenMenu {
    pub id: ComboId,
    /// Where the pointer was, in window coordinates. The menu opens here.
    pub anchor: Point<Pixels>,
    /// Optional element the menu hangs beneath instead, for a menu button.
    pub under: Option<Bounds<Pixels>>,
    /// What was clicked, in whatever form the host wants it back: a row id,
    /// a file path, a version hash. The menu never looks inside it.
    pub target: SharedString,
    pub opened_at: Instant,
}

/// Per-view control state. `Default` is the empty state, which is also the
/// right starting point: nothing open, nothing animating, nothing dragged.
#[derive(Default)]
pub struct ControlState {
    /// The pop-up whose list is open, if any.
    pub open_combo: Option<ComboId>,
    /// When that list started revealing.
    pub combo_opened_at: Option<Instant>,
    /// A list fading back out, and when that began. It keeps rendering,
    /// without taking clicks, until the fade finishes.
    pub combo_closing: Option<(ComboId, Instant)>,
    /// A pop-up just closed by a press outside it. The same press's click
    /// must not reopen it when it landed on that pop-up's own button.
    pub combo_dismissed: Option<ComboId>,
    /// When `combo_dismissed` was set. The release that should clear it can
    /// land on an occluding surface and never reach the host, so the marker
    /// expires on its own too.
    pub combo_dismissed_at: Option<Instant>,

    /// Element id to the instant its state last changed, for the controls
    /// that animate between two states. Absent means "never changed", which
    /// is what keeps a first paint or a remount from animating.
    pub anim: HashMap<ElementId, Instant>,

    /// Scrollbar thumb drag in flight.
    pub scroll_drag: Option<ScrollDrag>,

    /// Slider, split or colour-pad drag in flight.
    pub track_drag: Option<TrackDrag>,
    /// Each track's bounds, recorded as it paints. A drag needs them long
    /// after the pointer has left the track.
    pub track_bounds: HashMap<ComboId, Bounds<Pixels>>,

    /// Tab reorder in flight.
    pub tab_drag: Option<TabDrag>,
    /// Each tab bar's per-tab bounds, in bar order, recorded as they paint.
    pub tab_bounds: HashMap<ComboId, Vec<Bounds<Pixels>>>,

    /// The open context menu, if any.
    pub menu: Option<OpenMenu>,

    /// The shortcut recorder waiting for a key chord, if any.
    pub recording: Option<ComboId>,
}

impl ControlState {
    pub fn new() -> Self {
        Self::default()
    }

    // ---- Pop-ups ----

    pub fn is_combo_open(&self, combo: ComboId) -> bool {
        self.open_combo == Some(combo)
    }

    /// Closes the open pop-up, letting its list fade back out the way it
    /// faded in.
    pub fn close_combo(&mut self) {
        if let Some(combo) = self.open_combo.take() {
            self.combo_closing = Some((combo, Instant::now()));
        }
    }

    /// Opens one pop-up, closing whichever was open.
    pub fn open_combo(&mut self, combo: ComboId) {
        self.close_combo();
        self.open_combo = Some(combo);
        self.combo_opened_at = Some(Instant::now());
        self.combo_closing = None;
    }

    // ---- Animation ----

    /// Marks an element as having just changed state, starting its
    /// animation.
    pub fn mark_changed(&mut self, id: impl Into<ElementId>) {
        self.anim.insert(id.into(), Instant::now());
    }

    /// How far through its animation an element is, 0..=1. An element that
    /// never changed reads as finished, so it paints its end state.
    pub fn anim_progress(&self, id: &ElementId, duration: Duration) -> f32 {
        self.anim
            .get(id)
            .map(|since| (since.elapsed().as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0))
            .unwrap_or(1.0)
    }

    /// True while a fade or slide still needs frames. A host folds this into
    /// whatever decides to request the next frame.
    pub fn animating(&self) -> bool {
        if self
            .anim
            .values()
            .any(|since| since.elapsed() < SWITCH_SLIDE)
        {
            return true;
        }
        if self.open_combo.is_some()
            && self
                .combo_opened_at
                .is_some_and(|since| since.elapsed() < COMBO_REVEAL)
        {
            return true;
        }
        self.combo_closing
            .is_some_and(|(_, since)| since.elapsed() < COMBO_REVEAL)
    }

    /// Drops animations that have finished. Optional housekeeping: the map
    /// is keyed by element id and so is bounded by the number of animated
    /// controls, but a host that builds ids from data can call this to keep
    /// it from growing.
    pub fn prune_anims(&mut self) {
        self.anim.retain(|_, since| since.elapsed() < SWITCH_SLIDE);
    }

    // ---- Track drags ----

    pub fn scroll_dragging(&self) -> bool {
        self.scroll_drag.is_some()
    }

    pub fn track_dragging(&self) -> bool {
        self.track_drag.is_some()
    }

    /// Whether a particular track is the one being dragged.
    pub fn is_dragging(&self, id: ComboId) -> bool {
        self.track_drag.as_ref().is_some_and(|drag| drag.id == id)
    }

    /// Starts a drag on a track, recording how it should be read.
    pub fn begin_track_drag(&mut self, id: ComboId, axis: TrackAxis, stops: Option<u32>) {
        self.track_drag = Some(TrackDrag { id, axis, stops });
    }

    /// Where along the dragged track a pointer position falls, each
    /// component 0..=1 and already snapped to the track's stops. `None`
    /// when nothing is being dragged, or when the track has not painted yet
    /// and so has no bounds.
    ///
    /// Both components are always filled in; a horizontal track's `y` is
    /// simply not interesting.
    pub fn track_ratio_at(&self, position: Point<Pixels>) -> Option<(ComboId, Point<f32>)> {
        let drag = self.track_drag.as_ref()?;
        let bounds = self.track_bounds.get(drag.id)?;
        let width = f32::from(bounds.size.width);
        let height = f32::from(bounds.size.height);
        if width <= 0.0 || height <= 0.0 {
            return None;
        }
        let snap = |ratio: f32| match drag.stops {
            Some(stops) if stops >= 2 => {
                let last = (stops - 1) as f32;
                (ratio * last).round() / last
            }
            _ => ratio,
        };
        let x = ((f32::from(position.x) - f32::from(bounds.origin.x)) / width).clamp(0.0, 1.0);
        let y = ((f32::from(position.y) - f32::from(bounds.origin.y)) / height).clamp(0.0, 1.0);
        // A pad snaps on both axes; a one-dimensional track only on its own,
        // so the unused component stays exact for a host that reads it.
        let point = match drag.axis {
            TrackAxis::Horizontal => Point::new(snap(x), y),
            TrackAxis::Vertical => Point::new(x, snap(y)),
            TrackAxis::Both => Point::new(snap(x), snap(y)),
        };
        Some((drag.id, point))
    }

    // ---- Tab drags ----

    /// Which tab in a bar sits under a pointer position, if any.
    pub fn tab_at(&self, bar: ComboId, position: Point<Pixels>) -> Option<usize> {
        self.tab_bounds
            .get(bar)?
            .iter()
            .position(|bounds| bounds.contains(&position))
    }

    /// Updates a tab drag for a new pointer position, returning whether the
    /// landing slot changed.
    ///
    /// A tab moves only once the pointer is past the midpoint of its
    /// neighbour, so a tab never swaps back and forth under a still hand.
    pub fn drag_tab_to(&mut self, position: Point<Pixels>) -> bool {
        let Some(drag) = &self.tab_drag else {
            return false;
        };
        let Some(bounds) = self.tab_bounds.get(drag.bar) else {
            return false;
        };
        let Some(current) = bounds.get(drag.to) else {
            return false;
        };
        let pointer = f32::from(position.x);
        let mut target = None;
        if pointer < f32::from(current.left())
            && drag.to > 0
            && let Some(previous) = bounds.get(drag.to - 1)
            && pointer < f32::from(previous.center().x)
        {
            target = Some(drag.to - 1);
        }
        if pointer > f32::from(current.right())
            && let Some(next) = bounds.get(drag.to + 1)
            && pointer > f32::from(next.center().x)
        {
            target = Some(drag.to + 1);
        }
        let drag = self.tab_drag.as_mut().expect("checked above");
        drag.moved = true;
        match target {
            Some(to) => {
                drag.to = to;
                true
            }
            None => false,
        }
    }

    // ---- Context menus ----

    /// Opens a context menu at a pointer position. `target` is the host's
    /// own note of what was clicked; it comes back untouched.
    pub fn open_menu(
        &mut self,
        id: ComboId,
        anchor: Point<Pixels>,
        target: impl Into<SharedString>,
    ) {
        self.close_combo();
        self.menu = Some(OpenMenu {
            id,
            anchor,
            under: None,
            target: target.into(),
            opened_at: Instant::now(),
        });
    }

    /// Opens a menu hanging beneath an element rather than at the pointer,
    /// for a button that drops a menu.
    pub fn open_menu_under(
        &mut self,
        id: ComboId,
        under: Bounds<Pixels>,
        target: impl Into<SharedString>,
    ) {
        self.close_combo();
        self.menu = Some(OpenMenu {
            id,
            anchor: under.origin,
            under: Some(under),
            target: target.into(),
            opened_at: Instant::now(),
        });
    }

    pub fn close_menu(&mut self) {
        self.menu = None;
    }

    pub fn is_menu_open(&self, id: ComboId) -> bool {
        self.menu.as_ref().is_some_and(|menu| menu.id == id)
    }

    /// What the open menu was summoned on, if any.
    pub fn menu_target(&self) -> Option<&str> {
        self.menu.as_ref().map(|menu| menu.target.as_ref())
    }

    // ---- Drag lifecycle ----

    /// Whether any gesture owned by the controls is in flight. A host with
    /// drags of its own ORs this with them.
    pub fn dragging_anything(&self) -> bool {
        self.scroll_drag.is_some() || self.track_drag.is_some() || self.tab_drag.is_some()
    }

    /// Ends every drag and clears the dismissal marker. Call it from the
    /// host's mouse-up, and from a mouse-move with no button held: a release
    /// outside the window never arrives, so without that a drag would still
    /// be running when the pointer comes back.
    ///
    /// Returns the finished tab reorder, if the gesture was one and it
    /// actually moved, so the host can apply it.
    pub fn end_drag(&mut self) -> Option<(ComboId, usize, usize)> {
        self.scroll_drag = None;
        self.track_drag = None;
        // A marker no toggle consumed (the click landed elsewhere) must not
        // eat some later toggle click.
        self.combo_dismissed = None;
        self.combo_dismissed_at = None;
        self.tab_drag
            .take()
            .filter(|drag| drag.moved && drag.from != drag.to)
            .map(|drag| (drag.bar, drag.from, drag.to))
    }
}

/// A view that hosts vampir controls.
///
/// The two required methods hand out the view's [`ControlState`]. The rest
/// have defaults and matter only for the gestures that outlive a single
/// element: a drag continues long after the pointer has left the control
/// that started it, so the host has to pump it from wherever it tracks the
/// pointer at the root.
pub trait ControlHost: Sized + 'static {
    fn control_state(&self) -> &ControlState;
    fn control_state_mut(&mut self) -> &mut ControlState;

    /// A slider, split divider or colour pad moved. Both components of `at`
    /// run 0..=1 across the track and are already snapped to its stops;
    /// read `x` for a horizontal control, `y` for a vertical one, both for
    /// a pad.
    ///
    /// Fires on the press as well as on every move, so a click anywhere on
    /// a track jumps there. Implement it once and route on `id`.
    fn track_dragged(&mut self, _id: ComboId, _at: Point<f32>, _cx: &mut Context<Self>) {}

    /// A tab was dropped in a new slot. `from` and `to` are indices into the
    /// bar's items.
    fn tabs_reordered(&mut self, _bar: ComboId, _from: usize, _to: usize, _cx: &mut Context<Self>) {
    }

    /// Forwarded from a surface that blocks the mouse, such as a scrollbar
    /// track. While the pointer is over one, the host sees neither moves nor
    /// releases, so the surface passes them through here.
    fn forwarded_mouse_move(
        &mut self,
        _event: &MouseMoveEvent,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    /// Forwarded from a surface that blocks the mouse.
    fn forwarded_mouse_up(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}
}

/// Pumps every in-flight control drag from one pointer position. Call it
/// from the host's mouse-move tracking; it does nothing when nothing is
/// being dragged, so it is safe to call unconditionally.
///
/// Returns whether anything moved, so the host can skip a redraw.
pub fn continue_drags<V: ControlHost>(
    host: &mut V,
    position: Point<Pixels>,
    cx: &mut Context<V>,
) -> bool {
    let mut moved = false;

    if let Some(drag) = &host.control_state().scroll_drag {
        let along = match drag.axis {
            ScrollAxis::Vertical => f32::from(position.y),
            ScrollAxis::Horizontal => f32::from(position.x),
        };
        apply_scroll_drag(drag, along);
        moved = true;
    }
    if let Some((id, at)) = host.control_state().track_ratio_at(position) {
        host.track_dragged(id, at, cx);
        moved = true;
    }
    if host.control_state_mut().drag_tab_to(position) {
        moved = true;
    }
    moved
}

/// Ends every in-flight control drag, applying a finished tab reorder. Call
/// it from the host's mouse-up, and from a mouse-move with no button held.
pub fn end_drags<V: ControlHost>(host: &mut V, cx: &mut Context<V>) {
    if let Some((bar, from, to)) = host.control_state_mut().end_drag() {
        host.tabs_reordered(bar, from, to, cx);
    }
}
