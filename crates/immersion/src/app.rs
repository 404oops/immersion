//! RootView: owns the AppBackend, the theme, and all window-level UI state.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    Bounds, Context, Entity, Pixels, ScrollHandle, UniformListScrollHandle, Window, div, prelude::*,
};
use musit_core::backend::{AppBackend, BackendEvent, ColorSchemeMode, PlatformHooks};

use crate::theme::Theme;
use crate::ui::controls::{
    CONFIRM_DIALOG, DETAILS_SPLIT, HUE_SLIDER, LAYOUT_DIALOG, LIST_SPLIT, SATURATION_SLIDER,
};
use crate::ui::graph_layout::{self, GraphNode, Placement};
use vampir::text_input::{InputStyle, TextInput};
use vampir::{ControlHost, ControlState, Dismiss};

/// Pending confirm dialog (ThemedConfirmDialog).
pub struct ConfirmState {
    pub title: String,
    pub message: String,
    pub confirm_text: String,
    pub danger: bool,
    pub action: ConfirmAction,
}

pub enum ConfirmAction {
    ResetConfig,
    DeleteVersion(String),
    RemoveFolder(i32),
}

/// Onboarding wizard: pick a folder, choose its layout, watch the scan, then
/// optionally add more folders.
#[derive(Clone, Debug)]
pub enum OnboardingStep {
    Welcome,
    ChooseLayout { folder: String },
    Scanning { folder: String },
    AddMore { found_projects: bool },
}

/// Tab colours: switching tabs eases the theme hue across instead of
/// snapping, so the whole window shifts colour in one motion.
const HUE_ANIM_DURATION: Duration = Duration::from_millis(280);
/// Selection highlights cross-fade in this long: quicker than the colour
/// shift, so clicking around never feels laggy.
pub const SELECTION_FADE: Duration = Duration::from_millis(180);

/// Dragging the graph by its background: pans the view by the pointer's
/// travel since the press.
pub struct GraphPan {
    pub start_mouse: gpui::Point<Pixels>,
    pub start_offset: gpui::Point<Pixels>,
    /// Set once the pointer has travelled far enough to count as a drag,
    /// so a click that happens to land on the background is not a pan.
    pub moved: bool,
}

/// A selection moving from one item to another; the old one fades out while
/// the new one fades in.
pub struct SelectionFade<T> {
    pub to: T,
    pub from: T,
    pub since: Instant,
}

impl<T: PartialEq> SelectionFade<T> {
    pub fn running(&self) -> bool {
        self.since.elapsed() < SELECTION_FADE
    }

    /// Eased 0..=1 progress of the fade.
    pub fn progress(&self) -> f32 {
        let t = (self.since.elapsed().as_secs_f32() / SELECTION_FADE.as_secs_f32()).clamp(0.0, 1.0);
        crate::ui::controls::ease_out_cubic(t)
    }

    /// How selected `item` looks right now: fading in if it is the target,
    /// out if it was the previous selection, otherwise its steady state.
    pub fn weight(&self, item: &T, selected: bool) -> f32 {
        // Once the fade is over it has no say: the live selection decides.
        // Its keys can also go stale (a row index means a different project
        // after the list is filtered), which must not outlive the fade.
        if !self.running() {
            return if selected { 1.0 } else { 0.0 };
        }
        if *item == self.to {
            self.progress()
        } else if *item == self.from {
            1.0 - self.progress()
        } else if selected {
            1.0
        } else {
            0.0
        }
    }
}

/// Narrowest the list pane may get: its header row ("Discovered Projects" and
/// the Sort combo) stops fitting below this.
pub const LIST_PANE_MIN: f32 = 340.0;
/// Narrowest the version graph may get beside it.
pub const GRAPH_PANE_MIN: f32 = 300.0;

/// A colour cross-fade in flight. Both endpoints are fixed when it starts:
/// re-deriving the destination each frame made a tab switch mid-fade jump
/// straight to wherever the old fade's progress had reached.
pub struct HueAnim {
    pub from: Theme,
    pub to: Theme,
    pub started: Instant,
}

impl HueAnim {
    fn aims_at(&self, target: &Theme) -> bool {
        (self.to.hue - target.hue).abs() < 1e-9
            && (self.to.saturation - target.saturation).abs() < 1e-9
            && self.to.is_dark_mode == target.is_dark_mode
    }
}

/// Icons are rendered at this many pixels. The list tile is 34pt and the
/// trim keeps about 88% of the canvas, so 128px stays crisp on a 2x display
/// and downsamples cleanly on 1x.
const FILE_ICON_PIXELS: usize = 128;

/// Lowercase extension of a file name, empty when it has none.
fn file_extension(file_name: &str) -> String {
    match file_name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() && !extension.is_empty() => {
            extension.to_ascii_lowercase()
        }
        _ => String::new(),
    }
}

/// Signed distance from `from` to `to` the short way around the colour wheel.
fn shortest_hue_delta(from: f64, to: f64) -> f64 {
    (to - from + 540.0).rem_euclid(360.0) - 180.0
}

/// What the projects-folder layout dialog is asking about. Picking an
/// option only changes `selected_layout`; nothing is saved or scanned until
/// OK. Whether the dialog is showing is the toolkit's to say
/// (`ControlState::is_dialog_open(LAYOUT_DIALOG)`).
#[derive(Default)]
pub struct LayoutDialogState {
    pub folder_path: String,
    pub selected_layout: String,
    /// True when the folder is not a tab yet, so OK has to add it.
    pub is_new_folder: bool,
}

pub struct RootView {
    pub backend: AppBackend,
    pub theme: Theme,
    pub color_scheme: ColorSchemeMode,
    pub system_dark: bool,

    // Text inputs.
    pub search_input: Entity<TextInput>,
    pub project_note_input: Entity<TextInput>,
    pub version_note_input: Entity<TextInput>,
    pub retention_input: Entity<TextInput>,

    // Modals. Settings is the app's own panel; the layout and confirm
    // dialogs are vampir dialogs, so whether one is up lives in `controls`
    // and these only carry what it shows.
    pub settings_open: bool,
    pub layout_dialog: LayoutDialogState,
    /// What the confirm dialog is asking. Kept while the dialog fades out,
    /// so it has something to show, and cleared once it has gone.
    pub confirm: Option<ConfirmState>,

    // Version manager state.
    pub vm_graph: Vec<GraphNode>,
    pub vm_selected_id: String,
    pub vm_graph_extent: (f32, f32),
    /// Column count the graph was last laid out for (see `sync_graph_columns`).
    pub vm_graph_columns: usize,
    /// Pane width the graph was last laid out for, in content units (the pane
    /// divided by the zoom), so zooming reflows the columns.
    pub vm_graph_layout_w: f32,
    /// Gutter centres and dot-grid pitch from the last layout pass.
    pub vm_graph_placement: Placement,
    /// A drag of the graph in flight.
    pub graph_pan: Option<GraphPan>,
    /// Version whose node was pressed; it is selected on release, provided
    /// the press did not turn into a pan.
    pub pending_node_click: Option<String>,
    /// Text last loaded into each note field. A field whose content still
    /// equals this has not been edited, so a background refresh may replace
    /// it; once it differs the user is typing and their text is left alone.
    pub version_note_loaded: String,
    pub project_note_loaded: String,
    /// Project-row and version-node selection cross-fades in flight.
    pub row_fade: Option<SelectionFade<i32>>,
    pub node_fade: Option<SelectionFade<String>>,
    /// OS document icons by lowercase file extension; None once a lookup
    /// found nothing, so it is not retried. One small PNG per document type.
    pub file_icons: HashMap<String, Option<Arc<gpui::Image>>>,

    /// Everything the vampir controls keep between frames: which pop-up is
    /// open and how far into its fade, the switch and disclosure timers, and
    /// any scrollbar or slider drag.
    pub controls: ControlState,
    /// In-flight tab-to-tab colour shift (see `displayed_hue`).
    pub hue_anim: Option<HueAnim>,
    pub split_bounds: Option<Bounds<Pixels>>,
    /// Width share of the project list in the list|graph split: the list is
    /// a fixed-width card, the graph is what benefits from the extra room.
    pub list_fraction: f32,
    /// Bottom details panel height as a fraction of the split area.
    pub details_fraction: f32,
    /// Some while the first-run wizard is showing.
    pub onboarding: Option<OnboardingStep>,
    /// The layout picked on the wizard's layout step: an index into
    /// [`crate::ui::onboarding::LAYOUTS`].
    pub onboarding_layout: usize,
    /// Next event-driven graph refresh should select the latest version.
    pub pending_select_latest: bool,
    /// Folder tab currently being renamed inline (double-click a tab).
    pub renaming_tab: Option<i32>,
    pub tab_name_input: Entity<TextInput>,
    /// Version-graph zoom (1.0 = 100%).
    pub graph_zoom: f32,

    // Settings panel enter/exit (OutCubic / InCubic): Instant-driven opacity
    // so frame requests stop when the transition finishes (no stuck 60Hz
    // redraw).
    pub settings_enter_at: Option<Instant>,
    pub settings_exit_at: Option<Instant>,

    // Scroll handles.
    pub project_list_scroll: UniformListScrollHandle,
    pub activity_scroll: UniformListScrollHandle,
    pub settings_scroll: ScrollHandle,
    pub graph_scroll: ScrollHandle,
}

impl ControlHost for RootView {
    fn control_state(&self) -> &ControlState {
        &self.controls
    }

    fn control_state_mut(&mut self) -> &mut ControlState {
        &mut self.controls
    }

    /// The theme hue and saturation sliders and the two split dividers.
    fn track_dragged(&mut self, id: vampir::ComboId, at: gpui::Point<f32>, cx: &mut Context<Self>) {
        match id {
            HUE_SLIDER => self.set_hue_from_ratio(at.x, cx),
            SATURATION_SLIDER => self.set_saturation_from_ratio(at.x, cx),
            LIST_SPLIT => self.set_list_split(at.x),
            DETAILS_SPLIT => self.set_details_split(at.y),
            _ => {}
        }
    }

    /// A folder tab was dropped in a new slot.
    fn tabs_reordered(
        &mut self,
        _bar: vampir::ComboId,
        from: usize,
        to: usize,
        cx: &mut Context<Self>,
    ) {
        self.backend.move_folder_tab(from as i32, to as i32);
        cx.notify();
    }

    /// Scrollbar tracks block the mouse, so while the pointer is over one the
    /// root sees neither moves nor releases. They forward both here.
    fn forwarded_mouse_move(
        &mut self,
        event: &gpui::MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.global_mouse_move(event, cx);
    }

    fn forwarded_mouse_up(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.global_mouse_up(cx);
    }
}

impl RootView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let platform_hooks = PlatformHooks {
            launch_at_startup_supported: crate::platform::is_launch_at_startup_supported(),
            launch_at_startup_enabled: crate::platform::launch_at_startup_enabled(),
            set_launch_at_startup: Box::new(crate::platform::set_launch_at_startup),
            ..PlatformHooks::default()
        };
        let backend = AppBackend::with_platform(platform_hooks);

        let system_dark = vampir::theme::system_dark(window);
        let color_scheme = backend.color_scheme_mode();
        let dark = match color_scheme {
            ColorSchemeMode::Dark => true,
            ColorSchemeMode::Light => false,
            ColorSchemeMode::System => system_dark,
        };
        let theme = Theme::compute(backend.theme_hue(), backend.theme_saturation(), dark);

        let search_input =
            cx.new(|cx| TextInput::new(cx, "Search projects...", false, field_style(&theme)));
        let project_note_input = cx.new(|cx| {
            TextInput::new(
                cx,
                "Write a note for this project...",
                true,
                area_style(&theme),
            )
        });
        let version_note_input = cx.new(|cx| {
            TextInput::new(
                cx,
                "Write a note for selected version...",
                true,
                area_style(&theme),
            )
        });
        let retention_input = cx.new(|cx| {
            let mut input = TextInput::new(cx, "", false, field_style(&theme));
            input.set_text(&backend.snapshot_retention().to_string(), cx);
            input
        });
        let tab_name_input =
            cx.new(|cx| TextInput::new(cx, "Tab name", false, field_style(&theme)));

        // Enter commits the inline tab rename.
        {
            let weak = cx.entity().downgrade();
            tab_name_input.update(cx, |input, _| {
                input.on_submit = Some(Box::new(move |text, cx| {
                    let text = text.to_string();
                    if let Some(root) = weak.upgrade() {
                        root.update(cx, |root, cx| {
                            if let Some(index) = root.renaming_tab.take() {
                                root.backend.rename_projects_folder(index, &text);
                            }
                            cx.notify();
                        });
                    }
                }));
            });
        }

        // Retention spinbox direct entry: commit values within range.
        {
            let weak = cx.entity().downgrade();
            retention_input.update(cx, |input, _| {
                input.on_change = Some(Box::new(move |text, cx| {
                    let Ok(value) = text.trim().parse::<i32>() else {
                        return;
                    };
                    if !(1..=50).contains(&value) {
                        return;
                    }
                    if let Some(root) = weak.upgrade() {
                        root.update(cx, |root, _| root.backend.set_snapshot_retention(value));
                    }
                }));
            });
        }

        // Search field pushes straight into the backend (onTextEdited).
        {
            let weak = cx.entity().downgrade();
            search_input.update(cx, |input, _| {
                input.on_change = Some(Box::new(move |text, cx| {
                    let text = text.to_string();
                    if let Some(root) = weak.upgrade() {
                        root.update(cx, |root, _| root.backend.set_search_text(&text));
                    }
                }));
            });
        }

        // Backend pump.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                let alive = this
                    .update(cx, |root: &mut RootView, cx| root.pump(cx))
                    .is_ok();
                if !alive {
                    break;
                }
            }
        })
        .detach();

        // Track macOS light/dark appearance for "System" scheme.
        window
            .observe_window_appearance({
                let weak = cx.entity().downgrade();
                move |window, cx| {
                    let dark = vampir::theme::system_dark(window);
                    if let Some(root) = weak.upgrade() {
                        root.update(cx, |root, cx| {
                            root.system_dark = dark;
                            root.recompute_theme(cx);
                        });
                    }
                }
            })
            .detach();

        let mut this = Self {
            backend,
            theme,
            color_scheme,
            system_dark,
            search_input,
            project_note_input,
            version_note_input,
            retention_input,
            settings_open: false,
            layout_dialog: LayoutDialogState::default(),
            confirm: None,
            vm_graph: Vec::new(),
            vm_selected_id: String::new(),
            vm_graph_extent: (0.0, 0.0),
            vm_graph_columns: 0,
            vm_graph_layout_w: 0.0,
            vm_graph_placement: Placement::default(),
            graph_pan: None,
            pending_node_click: None,
            version_note_loaded: String::new(),
            project_note_loaded: String::new(),
            row_fade: None,
            node_fade: None,
            file_icons: HashMap::new(),
            controls: ControlState::new(),
            hue_anim: None,
            split_bounds: None,
            list_fraction: 0.4,
            details_fraction: 0.3,
            onboarding: None,
            onboarding_layout: 0,
            pending_select_latest: false,
            renaming_tab: None,
            tab_name_input,
            graph_zoom: 1.0,
            settings_enter_at: None,
            settings_exit_at: None,
            project_list_scroll: UniformListScrollHandle::new(),
            activity_scroll: UniformListScrollHandle::new(),
            settings_scroll: ScrollHandle::new(),
            graph_scroll: ScrollHandle::new(),
        };

        // First run (no folders yet): show the onboarding wizard. A saved
        // folder missing its layout choice re-enters the wizard at that step.
        let pending = this.backend.pending_projects_folder_setup().to_string();
        if !pending.is_empty() {
            this.onboarding = Some(OnboardingStep::ChooseLayout { folder: pending });
        } else if !this.backend.has_projects_folder() {
            this.onboarding = Some(OnboardingStep::Welcome);
        }

        this
    }

    // ---- Backend event pump ---------------------------------------------

    /// Persist debounced settings before the process exits.
    pub fn flush_before_quit(&mut self) {
        self.backend.flush_pending_persist();
    }

    fn pump(&mut self, cx: &mut Context<Self>) {
        self.backend.process_pending();
        if !self.backend.has_pending_events() {
            return;
        }
        let events = self.backend.take_events();

        // Coalesce idempotent reactions so a batch of queued events (e.g.
        // several graph-changed notices from one bundle save) triggers each
        // expensive refresh at most once per pump tick.
        let mut refresh_graph = false;
        let mut sync_project_note = false;
        let mut sync_search = false;
        let mut sync_retention = false;
        let mut scroll_activity = false;
        let mut recompute_theme = false;

        for event in events {
            match event {
                BackendEvent::SearchTextChanged => sync_search = true,
                BackendEvent::ActivityChanged => scroll_activity = true,
                BackendEvent::SnapshotRetentionChanged => sync_retention = true,
                BackendEvent::SelectedProjectVersionGraphChanged => refresh_graph = true,
                BackendEvent::SelectedProjectNoteChanged => sync_project_note = true,
                BackendEvent::ThemeHueChanged => {
                    // While dragging the hue slider the synchronous update
                    // already recomputed the theme; skip the echo. Every other
                    // source (switching tabs, most of all) eases across.
                    if !self.controls.is_dragging(HUE_SLIDER) {
                        self.start_hue_animation();
                        recompute_theme = true;
                    }
                }
                BackendEvent::ThemeSaturationChanged => {
                    if !self.controls.is_dragging(SATURATION_SLIDER) {
                        self.start_hue_animation();
                        recompute_theme = true;
                    }
                }
                BackendEvent::ColorSchemeModeChanged => {
                    self.color_scheme = self.backend.color_scheme_mode();
                    recompute_theme = true;
                }
                BackendEvent::PendingProjectsFolderSetupChanged => {
                    let pending = self.backend.pending_projects_folder_setup().to_string();
                    if !pending.is_empty() {
                        self.onboarding = Some(OnboardingStep::ChooseLayout { folder: pending });
                    }
                }
                BackendEvent::ProjectsFolderScanFinished {
                    folder,
                    found_projects,
                } => {
                    // Wizard: the scan *we* kicked off finished; a background
                    // folder finishing its own scan must not advance the step.
                    if let Some(OnboardingStep::Scanning { folder: awaited }) = &self.onboarding
                        && musit_core::path_cleanup::path_equals(awaited, &folder)
                    {
                        self.onboarding = Some(OnboardingStep::AddMore { found_projects });
                    }
                }
                BackendEvent::ConfigReset => {
                    self.settings_open = false;
                    self.settings_enter_at = None;
                    self.settings_exit_at = None;
                    self.onboarding = Some(OnboardingStep::Welcome);
                    self.onboarding_layout = 0;
                    self.renaming_tab = None;
                    self.controls.close_dialog();
                    self.confirm = None;
                }
                BackendEvent::ProjectSaveRecorded {
                    project_name,
                    version_label,
                    relative_path,
                }
                    // Desktop notification for a recorded save.
                    if self.backend.notifications_enabled() => {
                        let artifact_path =
                            musit_core::backup_template::artifact_for_path(&relative_path);
                        let artifact_name = musit_core::path_cleanup::file_name(&artifact_path);
                        let body = if artifact_name.is_empty() {
                            format!("{project_name} saved {version_label}")
                        } else {
                            format!("{project_name} \u{2014} {artifact_name} saved {version_label}")
                        };
                        crate::platform::show_notification("Snapshot saved", &body);
                    }
                _ => {}
            }
        }

        if sync_search {
            // Keep the search box in sync when the backend clears the
            // filter (folder change, reset).
            let text = self.backend.search_text().to_string();
            self.search_input.update(cx, |input, cx| {
                if input.text() != text {
                    input.set_text(&text, cx);
                }
            });
        }
        if scroll_activity && !self.backend.activity().is_empty() {
            self.activity_scroll.scroll_to_bottom();
        }
        if sync_retention {
            let text = self.backend.snapshot_retention().to_string();
            self.retention_input.update(cx, |input, cx| {
                if input.text().trim().parse::<i32>() != text.parse::<i32>() {
                    input.set_text(&text, cx);
                }
            });
        }
        if recompute_theme {
            self.recompute_theme(cx);
        }
        if refresh_graph {
            let select_latest = std::mem::take(&mut self.pending_select_latest);
            self.refresh_version_graph(select_latest, cx);
        }
        if sync_project_note {
            let note = self.backend.selected_project_note().to_string();
            let input = self.project_note_input.clone();
            let mut loaded = std::mem::take(&mut self.project_note_loaded);
            Self::load_note_field(&input, &mut loaded, &note, true, false, cx);
            self.project_note_loaded = loaded;
        }
        cx.notify();
    }

    /// Developer detail (on-disk layout: `.immersion/settings.json`, `.musit`
    /// folders) is spelled out only when the activity log is set to Debug.
    pub fn show_dev_details(&self) -> bool {
        self.backend.log_level() == "Debug"
    }

    // ---- Theme -----------------------------------------------------------

    /// Eased progress of the colour shift, 0.0..=1.0 (ease-out cubic, as the
    /// modal fades use).
    fn hue_progress(&self) -> f32 {
        let Some(anim) = &self.hue_anim else {
            return 1.0;
        };
        let elapsed = anim.started.elapsed().as_secs_f64();
        let duration = HUE_ANIM_DURATION.as_secs_f64();
        if elapsed >= duration {
            return 1.0;
        }
        (1.0 - (1.0 - elapsed / duration).powi(3)) as f32
    }

    fn theme_target(&self) -> Theme {
        Theme::compute(
            self.backend.theme_hue(),
            self.backend.theme_saturation(),
            self.dark_mode(),
        )
    }

    /// Starts (or retargets) the cross-fade towards the backend's colour.
    /// Snapshots the palette on screen right now, so a switch part-way
    /// through a fade sets off from the colour the user is looking at.
    fn start_hue_animation(&mut self) {
        let target = self.theme_target();
        // Already on its way there: leave the fade running rather than
        // restarting its clock.
        if self
            .hue_anim
            .as_ref()
            .is_some_and(|anim| anim.aims_at(&target))
        {
            return;
        }
        if shortest_hue_delta(self.theme.hue, target.hue).abs() < 0.5
            && (self.theme.saturation - target.saturation).abs() < 0.005
            && self.theme.is_dark_mode == target.is_dark_mode
        {
            self.hue_anim = None;
            return;
        }
        self.hue_anim = Some(HueAnim {
            from: self.theme,
            to: target,
            started: Instant::now(),
        });
    }

    fn hue_animating(&self) -> bool {
        self.hue_anim
            .as_ref()
            .is_some_and(|anim| anim.started.elapsed() < HUE_ANIM_DURATION)
    }

    pub fn recompute_theme(&mut self, cx: &mut Context<Self>) {
        self.apply_theme(cx);
        cx.notify();
    }

    fn dark_mode(&self) -> bool {
        match self.color_scheme {
            ColorSchemeMode::Dark => true,
            ColorSchemeMode::Light => false,
            ColorSchemeMode::System => self.system_dark,
        }
    }

    fn apply_theme(&mut self, cx: &mut Context<Self>) {
        let target = self.theme_target();
        // The colour moved while a fade was running (a tab switched
        // mid-fade): begin a fresh fade from the colour on screen.
        if self
            .hue_anim
            .as_ref()
            .is_some_and(|anim| !anim.aims_at(&target))
        {
            self.start_hue_animation();
        }
        self.theme = match &self.hue_anim {
            Some(anim) => Theme::blend(&anim.from, &anim.to, self.hue_progress()),
            None => target,
        };
        let field = field_style(&self.theme);
        let area = area_style(&self.theme);
        self.search_input.update(cx, |input, cx| {
            input.style = field;
            cx.notify();
        });
        self.project_note_input.update(cx, |input, cx| {
            input.style = area;
            cx.notify();
        });
        self.version_note_input.update(cx, |input, cx| {
            input.style = area;
            cx.notify();
        });
        self.retention_input.update(cx, |input, cx| {
            input.style = field;
            cx.notify();
        });
        self.tab_name_input.update(cx, |input, cx| {
            input.style = field;
            cx.notify();
        });
    }

    // ---- Modal helpers ----------------------------------------------------

    fn schedule_modal_exit(cx: &mut Context<Self>, finish: impl FnOnce(&mut RootView) + 'static) {
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(vampir::easing::MODAL_EXIT)
                .await;
            this.update(cx, |root, cx| {
                finish(root);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn request_close_settings(&mut self, cx: &mut Context<Self>) {
        if !self.settings_open || self.settings_exit_at.is_some() {
            return;
        }
        self.close_combo();
        self.settings_exit_at = Some(Instant::now());
        cx.notify();
        Self::schedule_modal_exit(cx, move |root| {
            // A reopen during the exit animation cleared exit_at; this close
            // is stale then and must not shut the fresh modal.
            if root.settings_exit_at.is_none() {
                return;
            }
            root.settings_open = false;
            root.settings_enter_at = None;
            root.settings_exit_at = None;
        });
    }

    pub fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Off any text input, so keys typed while the panel is up don't edit
        // fields behind the scrim.
        self.controls.focus_root(window, cx);
        // Discard any half-typed retention value from a previous visit.
        let retention = self.backend.snapshot_retention().to_string();
        self.retention_input
            .update(cx, |input, cx| input.set_text(&retention, cx));
        self.settings_open = true;
        self.settings_enter_at = Some(Instant::now());
        self.settings_exit_at = None;
    }

    /// Asks for confirmation. The dialog takes the keyboard itself when it
    /// paints, and hands it back on its way out.
    pub fn open_confirm(&mut self, confirm: ConfirmState) {
        self.confirm = Some(confirm);
        self.controls.open_dialog(CONFIRM_DIALOG);
    }

    /// Asks how a folder is laid out. Opening no longer scans: the folder is
    /// added (and scanned) once, when OK is pressed.
    pub fn open_layout_dialog_for_folder(&mut self, folder: &str) {
        let local = self.backend.display_local_path(folder);
        self.layout_dialog.selected_layout = self
            .backend
            .projects_folder_layout_for_path(&local)
            .to_string();
        self.layout_dialog.folder_path = local;
        self.layout_dialog.is_new_folder = true;
        self.controls.open_dialog(LAYOUT_DIALOG);
    }

    pub fn open_layout_dialog_for_current_folder(&mut self) {
        if self.backend.projects_folder_path().is_empty() {
            return;
        }
        self.layout_dialog.folder_path = self.backend.projects_folder_path().to_string();
        self.layout_dialog.is_new_folder = false;
        self.layout_dialog.selected_layout = self.backend.projects_folder_layout().to_string();
        self.controls.open_dialog(LAYOUT_DIALOG);
    }

    /// OK: saves the choice and rescans — the only path that does either.
    /// A folder already on this layout is left alone rather than rescanned.
    /// The dialog closes itself once this returns.
    pub fn layout_dialog_confirm(&mut self) {
        let local_path = self.layout_dialog.folder_path.clone();
        if local_path.is_empty() {
            return;
        }
        let layout = self.layout_dialog.selected_layout.clone();
        let unchanged = !self.layout_dialog.is_new_folder
            && self.backend.projects_folder_layout_for_path(&local_path) == layout;
        if !unchanged {
            self.pending_select_latest = true;
            self.backend.confirm_projects_folder(&local_path, &layout);
        }
    }

    pub fn layout_dialog_select(&mut self, layout: &str) {
        self.layout_dialog.selected_layout = layout.to_string();
    }

    /// Row click: select the project. The graph/details load once via the
    /// queued SelectedProject* events (no synchronous duplicate refresh).
    pub fn select_project(&mut self, visible_index: i32, cx: &mut Context<Self>) {
        self.pending_select_latest = true;
        let previous = self.backend.selected_project_index();
        if visible_index != previous {
            // A different project's note replaces the field.
            self.project_note_loaded = self.project_note_input.read(cx).text();
            self.row_fade = Some(SelectionFade {
                to: visible_index,
                from: previous,
                since: Instant::now(),
            });
            // A different graph: start it at the origin rather than inheriting
            // the previous project's pan.
            self.graph_scroll
                .set_offset(gpui::point(gpui::px(0.0), gpui::px(0.0)));
        }
        self.backend.set_selected_project_index(visible_index);
        cx.notify();
    }

    pub fn refresh_version_graph(&mut self, select_latest: bool, cx: &mut Context<Self>) {
        let previous_selected = self.vm_selected_id.clone();
        self.vm_graph = self
            .backend
            .selected_project_version_graph()
            .into_iter()
            .map(GraphNode::from)
            .collect();
        self.relayout_graph();

        if self.vm_graph.is_empty() {
            self.vm_selected_id = String::new();
            let input = self.version_note_input.clone();
            let mut loaded = std::mem::take(&mut self.version_note_loaded);
            Self::load_note_field(&input, &mut loaded, "", false, true, cx);
            self.version_note_loaded = loaded;
        } else {
            let exists = self
                .vm_graph
                .iter()
                .any(|node| node.version.id == self.vm_selected_id);
            if select_latest || self.vm_selected_id.is_empty() || !exists {
                self.vm_selected_id = self.default_selected_version_id();
            }
            let note = self
                .node_by_id(&self.vm_selected_id)
                .map(|node| node.version.note.clone())
                .unwrap_or_default();
            let has_selection = !self.vm_selected_id.is_empty();
            // The selection moved on its own (a restore, a deleted version):
            // that is a different subject, so the field is replaced. A plain
            // refresh of the same version keeps whatever is being typed.
            let force = self.vm_selected_id != previous_selected;
            let input = self.version_note_input.clone();
            let mut loaded = std::mem::take(&mut self.version_note_loaded);
            Self::load_note_field(&input, &mut loaded, &note, has_selection, force, cx);
            self.version_note_loaded = loaded;
        }
        self.sync_node_fade(&previous_selected);

        cx.notify();
    }

    /// Loads `text` into a note field unless the user has edited it since it
    /// was last loaded. `force` overrides that, for when the field is
    /// switching to a different subject and its old content no longer
    /// applies.
    fn load_note_field(
        input: &Entity<TextInput>,
        loaded: &mut String,
        text: &str,
        enabled: bool,
        force: bool,
        cx: &mut Context<Self>,
    ) {
        let untouched = input.read(cx).text() == *loaded;
        if force || untouched {
            let text = text.to_string();
            input.update(cx, |input, cx| {
                input.set_text(&text, cx);
                input.disabled = !enabled;
            });
            *loaded = text;
        } else {
            input.update(cx, |input, cx| {
                input.disabled = !enabled;
                cx.notify();
            });
        }
    }

    /// Selects a version in the graph: cross-fades the highlight and loads
    /// its note.
    pub fn select_version(&mut self, version_id: &str, cx: &mut Context<Self>) {
        if self.vm_selected_id == version_id {
            return;
        }
        // Explicit choice: a queued "select the newest version" must not
        // yank it away on the next refresh.
        self.pending_select_latest = false;
        let previous = std::mem::replace(&mut self.vm_selected_id, version_id.to_string());
        if !previous.is_empty() {
            self.node_fade = Some(SelectionFade {
                to: version_id.to_string(),
                from: previous,
                since: Instant::now(),
            });
        }
        let note = self
            .node_by_id(version_id)
            .map(|node| node.version.note.clone())
            .unwrap_or_default();
        // A different version's note replaces the field even mid-edit: the
        // text belonged to the version the user just left.
        let input = self.version_note_input.clone();
        let mut loaded = std::mem::take(&mut self.version_note_loaded);
        Self::load_note_field(&input, &mut loaded, &note, true, true, cx);
        self.version_note_loaded = loaded;
        cx.notify();
    }

    /// Starts a node cross-fade when the selection moved to another version
    /// of the same graph (a restore selects the restored version, say).
    fn sync_node_fade(&mut self, previous_selected: &str) {
        if previous_selected.is_empty()
            || self.vm_selected_id.is_empty()
            || previous_selected == self.vm_selected_id
        {
            return;
        }
        self.node_fade = Some(SelectionFade {
            to: self.vm_selected_id.clone(),
            from: previous_selected.to_string(),
            since: Instant::now(),
        });
    }

    /// The OS document icon for `file_name`, looked up (once) by extension.
    pub fn file_icon(&mut self, file_name: &str) -> Option<Arc<gpui::Image>> {
        let extension = file_extension(file_name);
        if extension.is_empty() {
            return None;
        }
        if let Some(cached) = self.file_icons.get(&extension) {
            return cached.clone();
        }
        let icon = crate::platform::file_type_icon_png(&extension, FILE_ICON_PIXELS)
            .map(|bytes| Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, bytes)));
        self.file_icons.insert(extension, icon.clone());
        icon
    }

    /// Cache-only variant of [`Self::file_icon`] for render paths that hold
    /// other borrows of the view; warm the cache with `file_icon` first.
    pub fn cached_file_icon(&self, file_name: &str) -> Option<Arc<gpui::Image>> {
        self.file_icons
            .get(&file_extension(file_name))
            .cloned()
            .flatten()
    }

    /// Closes the open combo, if any, letting its list fade back out the way
    /// it faded in.
    pub fn close_combo(&mut self) {
        self.controls.close_combo();
    }

    /// Pane width for the layout, measured last frame (zero before the first
    /// frame, which lays out a single column until the pane has a size) in
    /// content units at the current zoom. The zoom buttons are an overlay
    /// like the scrollbars; nodes may pass beneath them.
    fn graph_layout_width(&self) -> f32 {
        f32::from(self.graph_scroll.bounds().size.width) / self.graph_zoom.max(0.01)
    }

    /// Lays the graph out for the current wrap width and records the result.
    fn relayout_graph(&mut self) {
        let layout_w = self.graph_layout_width();
        let fitted = graph_layout::layout_for_pane(&mut self.vm_graph, layout_w);
        self.vm_graph_columns = fitted.columns;
        self.vm_graph_placement = fitted.placement;
        self.vm_graph_layout_w = layout_w;
        self.vm_graph_extent = graph_layout::extent(&self.vm_graph);
    }

    /// Re-flows the graph if the pane size changed (window or split resize,
    /// or a zoom step). Positions only; the selection is untouched.
    pub fn sync_graph_columns(&mut self) {
        if self.vm_graph.is_empty()
            || (self.graph_layout_width() - self.vm_graph_layout_w).abs() < 0.5
        {
            return;
        }
        self.relayout_graph();
    }

    pub fn node_by_id(&self, version_id: &str) -> Option<&GraphNode> {
        self.vm_graph
            .iter()
            .find(|node| node.version.id == version_id)
    }

    pub fn current_version_node(&self) -> Option<&GraphNode> {
        self.vm_graph.iter().find(|node| node.version.is_current)
    }

    fn default_selected_version_id(&self) -> String {
        if let Some(current) = self.current_version_node() {
            return current.version.id.clone();
        }
        let Some(mut latest) = self.vm_graph.first() else {
            return String::new();
        };
        for node in &self.vm_graph[1..] {
            if node.version.timestamp > latest.version.timestamp {
                latest = node;
            }
        }
        latest.version.id.clone()
    }

    /// Set of the selected node and all its ancestors (for link highlight).
    pub fn ancestor_id_set(&self, version_id: &str) -> std::collections::HashSet<String> {
        let mut ids = std::collections::HashSet::new();
        let mut current = version_id.to_string();
        while !current.is_empty() {
            ids.insert(current.clone());
            current = self
                .node_by_id(&current)
                .map(|node| node.parent_id.clone())
                .unwrap_or_default();
        }
        ids
    }

    /// Whether a vampir dialog is on screen, open or fading out.
    pub fn dialog_showing(&self) -> bool {
        self.controls.dialog_fade(LAYOUT_DIALOG).is_some()
            || self.controls.dialog_fade(CONFIRM_DIALOG).is_some()
    }

    /// True while any Instant-driven UI transition still needs frames.
    /// Hover uses GPUI `.hover()` (no RAF) so scrolling list rows cannot pin
    /// the display link at 60Hz.
    pub fn ui_animating(&self) -> bool {
        use crate::ui::controls::modal_opacity;

        // Switch slides, pop-up reveals, tab slides and dialog fades.
        if self.controls.animating() {
            return true;
        }
        if self.hue_animating() {
            return true;
        }
        if self.row_fade.as_ref().is_some_and(|fade| fade.running())
            || self.node_fade.as_ref().is_some_and(|fade| fade.running())
        {
            return true;
        }
        self.settings_open && modal_opacity(self.settings_enter_at, self.settings_exit_at).1
    }

    /// An Escape nothing of the toolkit's was open for: the root's handlers
    /// have already closed any pop-up list, and a dialog that had the
    /// keyboard has already taken it. What is left is the app's own.
    fn dismiss(&mut self, _: &Dismiss, _window: &mut Window, cx: &mut Context<Self>) {
        if self.renaming_tab.is_some() {
            // Escape cancels an inline tab rename.
            self.renaming_tab = None;
            cx.notify();
        } else if self.dialog_showing() {
            // A dialog on its way out passes Escape on; it must not close
            // the settings panel underneath as well.
        } else if self.settings_open {
            self.request_close_settings(cx);
        }
    }

    /// The confirm dialog's accepting button. The dialog closes itself once
    /// this returns; `confirm` stays for the fade and is cleared after.
    pub fn run_confirm_action(&mut self, cx: &mut Context<Self>) {
        if let Some(confirm) = &self.confirm {
            match &confirm.action {
                ConfirmAction::ResetConfig => {
                    self.backend.reset_config();
                }
                ConfirmAction::DeleteVersion(version_id) => {
                    // The backend pushes SelectedProjectVersionGraphChanged;
                    // the pump refreshes once.
                    self.backend.delete_version_by_id(version_id);
                }
                ConfirmAction::RemoveFolder(index) => {
                    self.pending_select_latest = true;
                    self.backend.remove_projects_folder(*index);
                }
            }
        }
        cx.notify();
    }
}

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Keep "System" scheme in sync even if the observer misses a change.
        let system_dark = vampir::theme::system_dark(window);
        if system_dark != self.system_dark {
            self.system_dark = system_dark;
            self.recompute_theme(cx);
        }

        if self.hue_anim.is_some() {
            if self.hue_animating() {
                self.apply_theme(cx);
            } else {
                self.hue_anim = None;
                self.apply_theme(cx);
            }
        }

        let theme = self.theme;
        let onboarding = self.onboarding.is_some();

        let body = if onboarding {
            self.render_onboarding(window, cx).into_any_element()
        } else {
            self.render_main_view(window, cx).into_any_element()
        };
        let modals = self.render_modal_layer(window, cx);

        // Tab, Shift-Tab and Escape are the toolkit's; it puts their handlers
        // on the root, and passes on an Escape it had nothing to close for.
        // The mouse is tracked here rather than by `vampir::root`, because
        // the graph pan is a drag of the app's own and shares the stream.
        let root = vampir::handle_keys(div().id("root"), self, cx)
            .size_full()
            .font_family(vampir::ui_font())
            .bg(crate::ui::lighting::lit(theme.app_background, 0.035))
            .text_color(theme.text_primary)
            .on_action(cx.listener(Self::dismiss))
            .on_mouse_move(
                cx.listener(|this, event: &gpui::MouseMoveEvent, _window, cx| {
                    this.global_mouse_move(event, cx);
                }),
            )
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, _event, _window, cx| {
                    this.global_mouse_up(cx);
                }),
            )
            .child(body)
            .children(modals);

        // Asked after the tree is built, as the toolkit requires: a control
        // that found its target moved while it was being built (a tab whose
        // width changed with its weight, say) has only just started its
        // tween, and asking before the build would miss it and leave it
        // stuck part-way until the next event happened to repaint.
        if self.ui_animating() {
            window.request_animation_frame();
        }

        root
    }
}

impl RootView {
    /// Applies a hue-slider position, `ratio` running 0..=1 along its track.
    fn set_hue_from_ratio(&mut self, ratio: f32, cx: &mut Context<Self>) {
        // Cap below 360: set_theme_hue wraps modulo 360, which would snap
        // the thumb from the right edge back to the left.
        let value = vampir::theme::Theme::hue_from_track(ratio)
            .round()
            .min(359.0);
        if (self.backend.theme_hue() - value).abs() < 0.5 {
            return;
        }
        self.hue_anim = None;
        self.backend.set_theme_hue(value);
        self.recompute_theme(cx);
    }

    /// Applies a saturation-slider position, `ratio` running 0..=1 along
    /// its track, which the toolkit maps over `0.0..=MAX_SATURATION`.
    fn set_saturation_from_ratio(&mut self, ratio: f32, cx: &mut Context<Self>) {
        // Whole percent steps, so the readout beside the slider and the
        // saved value agree.
        let value = (vampir::theme::Theme::saturation_from_track(ratio) * 100.0).round() / 100.0;
        if (self.backend.theme_saturation() - value).abs() < 0.005 {
            return;
        }
        self.hue_anim = None;
        self.backend.set_theme_saturation(value);
        self.recompute_theme(cx);
    }

    /// Whether any press-and-move gesture is in flight: the graph pan, or
    /// one of the toolkit's (a scrollbar thumb, the hue slider, a split
    /// divider, a folder tab).
    fn dragging_anything(&self) -> bool {
        self.graph_pan.is_some() || self.controls.dragging_anything()
    }

    /// Shared drag tracking. Attached to the root AND to occluding modal
    /// surfaces — an `.occlude()`d settings panel otherwise swallows the
    /// mouse-move stream the hue-slider drag depends on.
    pub fn global_mouse_move(&mut self, event: &gpui::MouseMoveEvent, cx: &mut Context<Self>) {
        // A release outside the window never reaches us, so a drag would
        // otherwise still be running when the pointer comes back. Any move
        // without the button held ends it, exactly as the release would
        // have.
        if !event.dragging() {
            if self.dragging_anything() {
                self.global_mouse_up(cx);
            }
            return;
        }

        // Scrollbar thumbs, the hue slider, the split dividers and the
        // folder tabs are the toolkit's own drags. A pressed tab only
        // becomes a drag once it has travelled `vampir::TAB_DRAG_THRESHOLD`,
        // so a click that wobbles a pixel still selects it.
        let mut dirty = vampir::continue_drags(self, event.position, cx);
        if let Some(pan) = &self.graph_pan {
            let dx = f32::from(event.position.x) - f32::from(pan.start_mouse.x);
            let dy = f32::from(event.position.y) - f32::from(pan.start_mouse.y);
            if !pan.moved && dx.abs().max(dy.abs()) < 3.0 {
                // Not yet a drag: leave the click to whatever is underneath.
            } else {
                let bounds = self.graph_scroll.bounds();
                let (extent_w, extent_h) = self.vm_graph_extent;
                let max_x = (extent_w * self.graph_zoom - f32::from(bounds.size.width)).max(0.0);
                let max_y = (extent_h * self.graph_zoom - f32::from(bounds.size.height)).max(0.0);
                let x = (f32::from(pan.start_offset.x) + dx).clamp(-max_x, 0.0);
                let y = (f32::from(pan.start_offset.y) + dy).clamp(-max_y, 0.0);
                self.graph_scroll
                    .set_offset(gpui::point(gpui::px(x), gpui::px(y)));
                if let Some(pan) = &mut self.graph_pan {
                    pan.moved = true;
                }
                dirty = true;
            }
        }
        if dirty {
            cx.notify();
        }
    }

    /// Sets the graph zoom, keeping `anchor` (window coords; viewport center
    /// when None) fixed in place by adjusting the scroll offset.
    /// Changes the graph zoom. The wrap width is derived from the zoom, so a
    /// step reflows the columns and no content point can be held under the
    /// cursor; instead the view keeps its position as a fraction of the
    /// scrollable range, which stays predictable in both directions.
    pub fn set_graph_zoom(&mut self, new_zoom: f32, cx: &mut Context<Self>) {
        let new_zoom = new_zoom.clamp(0.4, 3.0);
        let old_zoom = self.graph_zoom;
        if (new_zoom - old_zoom).abs() < 0.001 {
            return;
        }

        let bounds = self.graph_scroll.bounds();
        let viewport = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        let offset = self.graph_scroll.offset();
        let scrolled_fraction = |offset: f32, extent: f32, zoom: f32, viewport: f32| {
            let max = (extent * zoom - viewport).max(0.0);
            if max <= 0.0 {
                0.0
            } else {
                (-offset / max).clamp(0.0, 1.0)
            }
        };
        let (extent_w, extent_h) = self.vm_graph_extent;
        let fraction_x = scrolled_fraction(f32::from(offset.x), extent_w, old_zoom, viewport.0);
        let fraction_y = scrolled_fraction(f32::from(offset.y), extent_h, old_zoom, viewport.1);

        self.graph_zoom = new_zoom;
        self.relayout_graph();

        let (extent_w, extent_h) = self.vm_graph_extent;
        let max_x = (extent_w * new_zoom - viewport.0).max(0.0);
        let max_y = (extent_h * new_zoom - viewport.1).max(0.0);
        self.graph_scroll.set_offset(gpui::point(
            gpui::px(-fraction_x * max_x),
            gpui::px(-fraction_y * max_y),
        ));
        cx.notify();
    }

    pub fn global_mouse_up(&mut self, cx: &mut Context<Self>) {
        if self.controls.is_dragging(HUE_SLIDER) {
            self.backend.flush_pending_theme_hue_persist();
        }
        if self.controls.is_dragging(SATURATION_SLIDER) {
            self.backend.flush_pending_theme_saturation_persist();
        }
        if let Some(pan) = self.graph_pan.take() {
            // A press that never travelled is a click on the node under it.
            if !pan.moved
                && let Some(version_id) = self.pending_node_click.take()
                // The graph may have been rebuilt between press and release.
                && self.node_by_id(&version_id).is_some()
            {
                self.select_version(&version_id, cx);
            }
            cx.notify();
        }
        self.pending_node_click = None;
        // Ends the toolkit's drags (dropping a folder tab arrives at
        // `tabs_reordered`) and clears the dismissed-combo marker: one not
        // consumed by its toggle (the click landed elsewhere) must not eat a
        // later toggle click. Only a drag that was actually running needs a
        // repaint; every click in the window comes through here.
        let was_dragging = self.controls.dragging_anything();
        vampir::end_drags(self, cx);
        if was_dragging {
            cx.notify();
        }
    }

    /// The top-row|details divider moved to `at`, a fraction of the split
    /// area's height from its top. Clamped so neither pane collapses
    /// (matching the 210px render floor for the details panel).
    fn set_details_split(&mut self, at: f32) {
        let Some(bounds) = self.split_bounds else {
            return;
        };
        let total = f32::from(bounds.size.height);
        if total <= 0.0 {
            return;
        }
        let height = (total * (1.0 - at)).clamp(210.0, (total - 200.0).max(210.0));
        self.details_fraction = (height / total).clamp(0.15, 0.6);
    }

    /// The list|graph divider moved to `at`, a fraction of the split area's
    /// width. The list floor is what its own header needs ("Projects" and
    /// the Sort combo); below that the row overflows its panel.
    fn set_list_split(&mut self, at: f32) {
        let Some(bounds) = self.split_bounds else {
            return;
        };
        let total = f32::from(bounds.size.width);
        if total <= 0.0 {
            return;
        }
        let width = (total * at).clamp(LIST_PANE_MIN, (total - GRAPH_PANE_MIN).max(LIST_PANE_MIN));
        self.list_fraction = width / total;
    }
}

pub fn field_style(theme: &Theme) -> InputStyle {
    InputStyle {
        text_color: theme.text_primary,
        placeholder_color: theme.text_muted,
        selection_color: theme.selection,
        cursor_color: theme.accent,
        accent_color: theme.accent,
        font_size: 12.0,
        // Hand-picked colours; `apply_theme` re-styles the inputs itself.
        follows_palette: false,
    }
}

pub fn area_style(theme: &Theme) -> InputStyle {
    InputStyle {
        text_color: theme.vm_text_primary,
        placeholder_color: theme.text_muted,
        selection_color: theme.selection,
        cursor_color: theme.accent,
        accent_color: theme.accent,
        font_size: 13.0,
        follows_palette: false,
    }
}
