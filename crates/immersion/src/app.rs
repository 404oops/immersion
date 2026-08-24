//! RootView: owns the AppBackend, the theme, and all window-level UI state.
//! Mirrors the responsibilities of Main.qml + QmlBackend wiring.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, Context, ElementId, Entity, FocusHandle, Pixels, ScrollHandle,
    UniformListScrollHandle, Window, WindowAppearance, div, prelude::*,
};
use musit_core::backend::{
    AppBackend, BackendEvent, ColorSchemeMode, PlatformHooks, VersionGraphNode,
};

use crate::CloseModal;
use crate::text_input::{InputStyle, TextInput};
use crate::theme::Theme;

/// Which combo dropdown is currently open.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ComboId {
    SortMain,
    SortSettings,
    LogLevel,
    ColorScheme,
    PrimaryFile,
}

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
const HUE_ANIM_DURATION: Duration = Duration::from_millis(500);

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
        (self.to.hue - target.hue).abs() < 1e-9 && self.to.is_dark_mode == target.is_dark_mode
    }
}

/// Signed distance from `from` to `to` the short way around the colour wheel.
fn shortest_hue_delta(from: f64, to: f64) -> f64 {
    (to - from + 540.0).rem_euclid(360.0) - 180.0
}

/// ProjectsFolderLayoutDialog state. Picking a card only changes
/// `selected_layout`; nothing is saved or scanned until OK.
#[derive(Default)]
pub struct LayoutDialogState {
    pub open: bool,
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
    pub focus_handle: FocusHandle,

    // Text inputs.
    pub search_input: Entity<TextInput>,
    pub project_note_input: Entity<TextInput>,
    pub version_note_input: Entity<TextInput>,
    pub retention_input: Entity<TextInput>,

    // Modals.
    pub settings_open: bool,
    pub layout_dialog: LayoutDialogState,
    pub confirm: Option<ConfirmState>,

    // Version manager state.
    pub vm_graph: Vec<VersionGraphNode>,
    pub vm_selected_id: String,
    pub vm_graph_extent: (f32, f32),

    // Popup/interaction state.
    pub open_combo: Option<ComboId>,
    pub hue_slider_bounds: Option<Bounds<Pixels>>,
    pub hue_dragging: bool,
    /// In-flight tab-to-tab colour shift (see `displayed_hue`).
    pub hue_anim: Option<HueAnim>,
    pub split_bounds: Option<Bounds<Pixels>>,
    pub split_dragging: bool,
    /// Width share of the project list in the list|graph split: the list is
    /// a fixed-width card, the graph is what benefits from the extra room.
    pub list_fraction: f32,
    pub list_split_dragging: bool,
    /// Bottom details panel height as a fraction of the split area.
    pub details_fraction: f32,
    /// Some while the first-run wizard is showing.
    pub onboarding: Option<OnboardingStep>,
    /// Combo just closed by its popup's mouse-down-out; the toggle's click
    /// for that same gesture must not reopen it.
    pub combo_dismissed: Option<ComboId>,
    /// Next event-driven graph refresh should select the latest version.
    pub pending_select_latest: bool,
    /// Folder tab currently being renamed inline (double-click a tab).
    pub renaming_tab: Option<i32>,
    pub tab_name_input: Entity<TextInput>,
    /// Version-graph zoom (1.0 = 100%).
    pub graph_zoom: f32,

    // Animation state (ports of QML Behavior on color/x).
    // Switch knob/track slides: element id -> when it was last toggled.
    pub switch_anim: HashMap<ElementId, Instant>,
    // ThemedPopup enter/exit (OutCubic / InCubic): Instant-driven opacity so
    // frame requests stop when the transition finishes (no stuck 60Hz redraw).
    pub settings_enter_at: Option<Instant>,
    pub layout_enter_at: Option<Instant>,
    pub confirm_enter_at: Option<Instant>,
    pub settings_exit_at: Option<Instant>,
    pub layout_exit_at: Option<Instant>,
    pub confirm_exit_at: Option<Instant>,

    // Scroll handles.
    pub project_list_scroll: UniformListScrollHandle,
    pub activity_scroll: UniformListScrollHandle,
    pub settings_scroll: ScrollHandle,
    pub tabs_scroll: ScrollHandle,
    pub graph_scroll: ScrollHandle,
    pub layout_dialog_scroll: ScrollHandle,
    pub scroll_drag: Option<crate::ui::controls::ScrollDrag>,
}

impl RootView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let platform_hooks = PlatformHooks {
            launch_at_startup_supported: crate::platform::is_launch_at_startup_supported(),
            set_launch_at_startup: Box::new(crate::platform::set_launch_at_startup),
            ..PlatformHooks::default()
        };
        let backend = AppBackend::with_platform(platform_hooks);

        let system_dark = matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        let color_scheme = backend.color_scheme_mode();
        let dark = match color_scheme {
            ColorSchemeMode::Dark => true,
            ColorSchemeMode::Light => false,
            ColorSchemeMode::System => system_dark,
        };
        let theme = Theme::compute(backend.theme_hue(), dark);

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

        // Backend pump: replaces the Qt event loop.
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
                    let dark = matches!(
                        window.appearance(),
                        WindowAppearance::Dark | WindowAppearance::VibrantDark
                    );
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
            focus_handle: cx.focus_handle(),
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
            open_combo: None,
            hue_slider_bounds: None,
            hue_dragging: false,
            hue_anim: None,
            split_bounds: None,
            split_dragging: false,
            list_fraction: 0.4,
            list_split_dragging: false,
            details_fraction: 0.3,
            onboarding: None,
            combo_dismissed: None,
            pending_select_latest: false,
            renaming_tab: None,
            tab_name_input,
            graph_zoom: 1.0,
            switch_anim: HashMap::new(),
            settings_enter_at: None,
            layout_enter_at: None,
            confirm_enter_at: None,
            settings_exit_at: None,
            layout_exit_at: None,
            confirm_exit_at: None,
            project_list_scroll: UniformListScrollHandle::new(),
            activity_scroll: UniformListScrollHandle::new(),
            settings_scroll: ScrollHandle::new(),
            tabs_scroll: ScrollHandle::new(),
            graph_scroll: ScrollHandle::new(),
            layout_dialog_scroll: ScrollHandle::new(),
            scroll_drag: None,
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

    /// ~QmlBackend: persist debounced settings before the process exits.
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
                    if !self.hue_dragging {
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
                    self.renaming_tab = None;
                    self.layout_dialog.open = false;
                    self.layout_enter_at = None;
                    self.layout_exit_at = None;
                    self.confirm = None;
                    self.confirm_enter_at = None;
                    self.confirm_exit_at = None;
                }
                BackendEvent::ProjectSaveRecorded {
                    project_name,
                    version_label,
                    relative_path,
                } => {
                    // TrayController::onProjectSaveRecorded.
                    if self.backend.notifications_enabled() {
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
                }
                _ => {}
            }
        }

        if sync_search {
            // Keep the search box in sync when the backend clears the
            // filter (folder change, reset) — QML bound text both ways.
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
            self.project_note_input
                .update(cx, |input, cx| input.set_text(&note, cx));
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
        Theme::compute(self.backend.theme_hue(), self.dark_mode())
    }

    /// Starts (or retargets) the cross-fade towards the backend's colour.
    /// Snapshots the palette on screen right now, so a switch part-way
    /// through a fade sets off from the colour the user is looking at.
    fn start_hue_animation(&mut self) {
        let target = self.theme_target();
        // Already on its way there: leave the fade running rather than
        // restarting its clock.
        if self.hue_anim.as_ref().is_some_and(|anim| anim.aims_at(&target)) {
            return;
        }
        if shortest_hue_delta(self.theme.hue, target.hue).abs() < 0.5
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
        if self.hue_anim.as_ref().is_some_and(|anim| !anim.aims_at(&target)) {
            self.start_hue_animation();
        }
        self.theme = match &self.hue_anim {
            Some(anim) => Theme::lerp(&anim.from, &anim.to, self.hue_progress()),
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
                .timer(Duration::from_millis(crate::theme::MODAL_EXIT_DURATION_MS))
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
        self.open_combo = None;
        self.settings_exit_at = Some(Instant::now());
        cx.notify();
        Self::schedule_modal_exit(cx, |root| {
            // A reopen during the exit animation cleared exit_at; this close
            // is stale then and must not shut the fresh modal.
            if root.settings_exit_at.is_none() {
                return;
            }
            root.settings_open = false;
            root.settings_enter_at = None;
            root.settings_exit_at = None;
            root.layout_dialog.open = false;
            root.layout_enter_at = None;
            root.layout_exit_at = None;
        });
    }

    pub fn request_close_layout(&mut self, cx: &mut Context<Self>) {
        if !self.layout_dialog.open || self.layout_exit_at.is_some() {
            return;
        }
        self.layout_exit_at = Some(Instant::now());
        cx.notify();
        Self::schedule_modal_exit(cx, |root| {
            if root.layout_exit_at.is_none() {
                return;
            }
            root.layout_dialog.open = false;
            root.layout_enter_at = None;
            root.layout_exit_at = None;
        });
    }

    pub fn request_close_confirm(&mut self, cx: &mut Context<Self>) {
        if self.confirm.is_none() || self.confirm_exit_at.is_some() {
            return;
        }
        self.confirm_exit_at = Some(Instant::now());
        cx.notify();
        Self::schedule_modal_exit(cx, |root| {
            if root.confirm_exit_at.is_none() {
                return;
            }
            root.confirm = None;
            root.confirm_enter_at = None;
            root.confirm_exit_at = None;
        });
    }

    /// Moves keyboard focus off any text input onto the root view, so keys
    /// typed while a modal is up don't edit fields behind the scrim
    /// (ThemedPopup was `modal: true; focus: true`).
    pub fn take_modal_focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus_handle, cx);
    }

    pub fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.take_modal_focus(window, cx);
        // Discard any half-typed retention value from a previous visit.
        let retention = self.backend.snapshot_retention().to_string();
        self.retention_input
            .update(cx, |input, cx| input.set_text(&retention, cx));
        self.settings_open = true;
        self.settings_enter_at = Some(Instant::now());
        self.settings_exit_at = None;
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
        self.layout_dialog.open = true;
        self.layout_enter_at = Some(Instant::now());
        self.layout_exit_at = None;
    }

    pub fn open_layout_dialog_for_current_folder(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.backend.projects_folder_path().is_empty() {
            return;
        }
        self.take_modal_focus(window, cx);
        self.layout_dialog.folder_path = self.backend.projects_folder_path().to_string();
        self.layout_dialog.is_new_folder = false;
        self.layout_dialog.selected_layout = self.backend.projects_folder_layout().to_string();
        self.layout_dialog.open = true;
        self.layout_enter_at = Some(Instant::now());
        self.layout_exit_at = None;
    }

    /// OK: saves the choice and rescans — the only path that does either.
    /// A folder already on this layout is left alone rather than rescanned.
    pub fn layout_dialog_confirm(&mut self, cx: &mut Context<Self>) {
        let local_path = self.layout_dialog.folder_path.clone();
        if local_path.is_empty() {
            self.request_close_layout(cx);
            return;
        }
        let layout = self.layout_dialog.selected_layout.clone();
        let unchanged = !self.layout_dialog.is_new_folder
            && self.backend.projects_folder_layout_for_path(&local_path) == layout;
        if !unchanged {
            self.pending_select_latest = true;
            self.backend.confirm_projects_folder(&local_path, &layout);
        }
        self.request_close_layout(cx);
    }

    pub fn layout_dialog_select(&mut self, layout: &str) {
        self.layout_dialog.selected_layout = layout.to_string();
    }

    /// Row click: select the project. The graph/details load once via the
    /// queued SelectedProject* events (no synchronous duplicate refresh).
    pub fn select_project(&mut self, visible_index: i32, cx: &mut Context<Self>) {
        self.pending_select_latest = true;
        if visible_index != self.backend.selected_project_index() {
            // A different graph: start it at the origin rather than inheriting
            // the previous project's pan.
            self.graph_scroll
                .set_offset(gpui::point(gpui::px(0.0), gpui::px(0.0)));
        }
        self.backend.set_selected_project_index(visible_index);
        cx.notify();
    }

    pub fn refresh_version_graph(&mut self, select_latest: bool, cx: &mut Context<Self>) {
        self.vm_graph = self.backend.selected_project_version_graph();

        // applyVersionSelection
        if self.vm_graph.is_empty() {
            self.vm_selected_id = String::new();
            self.version_note_input.update(cx, |input, cx| {
                input.set_text("", cx);
                // VersionManagerWindow.qml: `enabled: selectedVersionId != ""`.
                input.disabled = true;
            });
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
            self.version_note_input.update(cx, |input, cx| {
                input.set_text(&note, cx);
                input.disabled = !has_selection;
            });
        }

        // Graph extent (VersionManagerWindow.refreshVersionGraph).
        if self.vm_graph.is_empty() {
            self.vm_graph_extent = (0.0, 0.0);
        } else {
            let mut max_x = self.vm_graph[0].x;
            let mut max_y = self.vm_graph[0].y;
            for node in &self.vm_graph[1..] {
                max_x = max_x.max(node.x);
                max_y = max_y.max(node.y);
            }
            // Node coordinates are absolute (the layout already leaves a margin
            // at the origin), so the extent is the far edge plus one pad — not
            // the span plus two.
            let node_half_w = 38.0;
            let node_half_h = 23.0;
            let viewport_pad = 32.0;
            self.vm_graph_extent = (
                max_x + node_half_w + viewport_pad,
                max_y + node_half_h + viewport_pad,
            );
        }
        cx.notify();
    }

    pub fn node_by_id(&self, version_id: &str) -> Option<&VersionGraphNode> {
        self.vm_graph
            .iter()
            .find(|node| node.version.id == version_id)
    }

    pub fn current_version_node(&self) -> Option<&VersionGraphNode> {
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

    pub fn any_modal_open(&self) -> bool {
        self.settings_open || self.layout_dialog.open
    }

    pub fn child_dialog_open(&self) -> bool {
        self.confirm.is_some() || (self.settings_open && self.layout_dialog.open)
    }

    /// True while any Instant-driven UI transition still needs frames.
    /// Hover uses GPUI `.hover()` (no RAF) so scrolling list rows cannot pin
    /// the display link at 60Hz.
    pub fn ui_animating(&self) -> bool {
        use crate::ui::controls::modal_opacity;

        const SWITCH_MAX: Duration = Duration::from_millis(140);

        if self
            .switch_anim
            .values()
            .any(|since| since.elapsed() < SWITCH_MAX)
        {
            return true;
        }
        if self.hue_animating() {
            return true;
        }
        if self.settings_open && modal_opacity(self.settings_enter_at, self.settings_exit_at).1 {
            return true;
        }
        if self.layout_dialog.open && modal_opacity(self.layout_enter_at, self.layout_exit_at).1 {
            return true;
        }
        if self.confirm.is_some() && modal_opacity(self.confirm_enter_at, self.confirm_exit_at).1 {
            return true;
        }
        false
    }

    fn close_modal(&mut self, _: &CloseModal, _window: &mut Window, cx: &mut Context<Self>) {
        if self.renaming_tab.is_some() {
            // Escape cancels an inline tab rename.
            self.renaming_tab = None;
            cx.notify();
        } else if self.open_combo.is_some() {
            self.open_combo = None;
            cx.notify();
        } else if self.confirm.is_some() {
            self.request_close_confirm(cx);
        } else if self.layout_dialog.open {
            self.request_close_layout(cx);
        } else if self.settings_open {
            self.request_close_settings(cx);
        }
    }

    pub fn run_confirm_action(&mut self, cx: &mut Context<Self>) {
        if let Some(confirm) = self.confirm.take() {
            match confirm.action {
                ConfirmAction::ResetConfig => {
                    self.backend.reset_config();
                }
                ConfirmAction::DeleteVersion(version_id) => {
                    // The backend pushes SelectedProjectVersionGraphChanged;
                    // the pump refreshes once.
                    self.backend.delete_version_by_id(&version_id);
                }
                ConfirmAction::RemoveFolder(index) => {
                    self.pending_select_latest = true;
                    self.backend.remove_projects_folder(index);
                }
            }
        }
        cx.notify();
    }
}

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Keep "System" scheme in sync even if the observer misses a change.
        let system_dark = matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
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

        if self.ui_animating() {
            window.request_animation_frame();
        }

        div()
            .id("root")
            .size_full()
            .font_family(".SystemUIFont")
            .bg(theme.app_background)
            .text_color(theme.text_primary)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::close_modal))
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
            .child(if onboarding {
                self.render_onboarding(window, cx).into_any_element()
            } else {
                self.render_main_view(window, cx).into_any_element()
            })
            .children(self.render_modal_layer(window, cx))
    }
}

impl RootView {
    pub fn update_hue_from_mouse(&mut self, position: gpui::Point<Pixels>, cx: &mut Context<Self>) {
        if let Some(bounds) = self.hue_slider_bounds {
            let width = f32::from(bounds.size.width);
            if width > 0.0 {
                let x = f32::from(position.x) - f32::from(bounds.origin.x);
                let ratio = (x / width).clamp(0.0, 1.0);
                // Cap below 360: set_theme_hue wraps modulo 360, which would
                // snap the thumb from the right edge back to the left.
                let value = (ratio * 360.0).round().min(359.0) as f64;
                if (self.backend.theme_hue() - value).abs() < 0.5 {
                    return;
                }
                self.hue_anim = None;
                self.backend.set_theme_hue(value);
                self.recompute_theme(cx);
            }
        }
    }

    /// Shared drag tracking. Attached to the root AND to occluding modal
    /// surfaces — an `.occlude()`d settings panel otherwise swallows the
    /// mouse-move stream the hue-slider drag depends on.
    pub fn global_mouse_move(&mut self, event: &gpui::MouseMoveEvent, cx: &mut Context<Self>) {
        let mut dirty = false;
        if self.hue_dragging {
            self.update_hue_from_mouse(event.position, cx);
            dirty = true;
        }
        if self.split_dragging {
            self.update_split_from_mouse(event.position);
            dirty = true;
        }
        if self.list_split_dragging {
            self.update_list_split_from_mouse(event.position);
            dirty = true;
        }
        if let Some(drag) = &self.scroll_drag {
            use crate::ui::controls::{ScrollAxis, apply_scroll_drag};
            let position = match drag.axis {
                ScrollAxis::Vertical => f32::from(event.position.y),
                ScrollAxis::Horizontal => f32::from(event.position.x),
            };
            apply_scroll_drag(drag, position);
            dirty = true;
        }
        if dirty {
            cx.notify();
        }
    }

    /// Sets the graph zoom, keeping `anchor` (window coords; viewport center
    /// when None) fixed in place by adjusting the scroll offset.
    pub fn set_graph_zoom(
        &mut self,
        new_zoom: f32,
        anchor: Option<gpui::Point<Pixels>>,
        cx: &mut Context<Self>,
    ) {
        let new_zoom = new_zoom.clamp(0.4, 3.0);
        let old_zoom = self.graph_zoom;
        if (new_zoom - old_zoom).abs() < 0.001 {
            return;
        }

        let bounds = self.graph_scroll.bounds();
        let anchor_in_viewport = match anchor {
            Some(position) => (
                f32::from(position.x) - f32::from(bounds.left()),
                f32::from(position.y) - f32::from(bounds.top()),
            ),
            None => (
                f32::from(bounds.size.width) / 2.0,
                f32::from(bounds.size.height) / 2.0,
            ),
        };
        let offset = self.graph_scroll.offset();
        let scale = new_zoom / old_zoom;
        // Content point under the anchor stays under the anchor.
        let content_x = anchor_in_viewport.0 - f32::from(offset.x);
        let content_y = anchor_in_viewport.1 - f32::from(offset.y);
        // Clamp against the scroll range the new zoom will produce, otherwise
        // gpui's own clamp snaps the offset back on the next prepaint and the
        // graph visibly jumps after a zoom-out.
        let viewport_w = f32::from(bounds.size.width);
        let viewport_h = f32::from(bounds.size.height);
        let (extent_w, extent_h) = self.vm_graph_extent;
        let max_scroll_x = (extent_w * new_zoom - viewport_w).max(0.0);
        let max_scroll_y = (extent_h * new_zoom - viewport_h).max(0.0);
        let new_offset_x = (anchor_in_viewport.0 - content_x * scale).clamp(-max_scroll_x, 0.0);
        let new_offset_y = (anchor_in_viewport.1 - content_y * scale).clamp(-max_scroll_y, 0.0);
        self.graph_scroll
            .set_offset(gpui::point(gpui::px(new_offset_x), gpui::px(new_offset_y)));

        self.graph_zoom = new_zoom;
        cx.notify();
    }

    pub fn global_mouse_up(&mut self, cx: &mut Context<Self>) {
        if self.hue_dragging {
            self.hue_dragging = false;
            self.backend.flush_pending_theme_hue_persist();
            cx.notify();
        }
        if self.split_dragging {
            self.split_dragging = false;
            cx.notify();
        }
        if self.list_split_dragging {
            self.list_split_dragging = false;
            cx.notify();
        }
        if self.scroll_drag.take().is_some() {
            cx.notify();
        }
        // A dismissed-combo marker not consumed by its toggle (e.g. the
        // click landed elsewhere) must not eat a later toggle click.
        self.combo_dismissed = None;
    }

    fn update_split_from_mouse(&mut self, position: gpui::Point<Pixels>) {
        if let Some(bounds) = self.split_bounds {
            let total = f32::from(bounds.size.height);
            if total <= 0.0 {
                return;
            }
            let bottom = f32::from(bounds.origin.y) + total;
            // 10px handle center offset; clamp so neither pane collapses
            // (matches the 210px render floor for the details panel).
            let height =
                (bottom - f32::from(position.y) - 10.0).clamp(210.0, (total - 200.0).max(210.0));
            self.details_fraction = (height / total).clamp(0.15, 0.6);
        }
    }

    /// The list|graph divider. Shares the split area's bounds with the
    /// details drag: the top row spans exactly the same x-range.
    fn update_list_split_from_mouse(&mut self, position: gpui::Point<Pixels>) {
        if let Some(bounds) = self.split_bounds {
            let total = f32::from(bounds.size.width);
            if total <= 0.0 {
                return;
            }
            // 5px handle half-width. The list floor is what its own header
            // needs ("Discovered Projects" + the Sort combo); below that the
            // row overflows its panel.
            let left = f32::from(bounds.origin.x);
            let width = (f32::from(position.x) - left - 5.0)
                .clamp(LIST_PANE_MIN, (total - GRAPH_PANE_MIN).max(LIST_PANE_MIN));
            self.list_fraction = width / total;
        }
    }
}

pub fn field_style(theme: &Theme) -> InputStyle {
    InputStyle {
        text_color: theme.text_primary,
        placeholder_color: theme.text_muted,
        selection_color: theme.selection,
        cursor_color: theme.accent,
        font_size: 12.0,
    }
}

pub fn area_style(theme: &Theme) -> InputStyle {
    InputStyle {
        text_color: theme.vm_text_primary,
        placeholder_color: theme.text_muted,
        selection_color: theme.selection,
        cursor_color: theme.accent,
        font_size: 13.0,
    }
}
