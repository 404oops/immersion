//! RootView: owns the AppBackend, the theme, and all window-level UI state.
//! Mirrors the responsibilities of Main.qml + QmlBackend wiring.

use std::time::Duration;

use gpui::{
    Bounds, Context, Entity, FocusHandle, Pixels, ScrollHandle, UniformListScrollHandle, Window,
    WindowAppearance, div, prelude::*,
};
use musit_core::backend::{
    AppBackend, BackendEvent, ColorSchemeMode, PlatformHooks, VersionGraphNode,
};

use crate::text_input::{InputStyle, TextInput};
use crate::theme::Theme;
use crate::CloseModal;

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
}

/// ProjectsFolderLayoutDialog state.
#[derive(Default)]
pub struct LayoutDialogState {
    pub open: bool,
    pub folder_path: String,
    pub selected_layout: String,
    pub awaiting_scan: bool,
    pub scan_error: String,
    pub scan_succeeded: bool,
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

    // Modals.
    pub settings_open: bool,
    pub vm_open: bool,
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
    pub split_bounds: Option<Bounds<Pixels>>,
    pub split_dragging: bool,
    pub activity_panel_height: f32,

    // Scroll handles.
    pub project_list_scroll: ScrollHandle,
    pub activity_scroll: UniformListScrollHandle,
    pub settings_scroll: ScrollHandle,
    pub vm_side_scroll: ScrollHandle,
    pub graph_scroll: ScrollHandle,
    pub layout_dialog_scroll: ScrollHandle,
}

impl RootView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let backend = AppBackend::with_platform(PlatformHooks::default());

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

        let search_input = cx.new(|cx| {
            TextInput::new(cx, "Search projects...", false, field_style(&theme))
        });
        let project_note_input = cx.new(|cx| {
            TextInput::new(cx, "Write a note for this project...", true, area_style(&theme))
        });
        let version_note_input = cx.new(|cx| {
            TextInput::new(
                cx,
                "Write a note for selected version...",
                true,
                area_style(&theme),
            )
        });

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
            settings_open: false,
            vm_open: false,
            layout_dialog: LayoutDialogState::default(),
            confirm: None,
            vm_graph: Vec::new(),
            vm_selected_id: String::new(),
            vm_graph_extent: (0.0, 0.0),
            open_combo: None,
            hue_slider_bounds: None,
            hue_dragging: false,
            split_bounds: None,
            split_dragging: false,
            activity_panel_height: 150.0,
            project_list_scroll: ScrollHandle::new(),
            activity_scroll: UniformListScrollHandle::new(),
            settings_scroll: ScrollHandle::new(),
            vm_side_scroll: ScrollHandle::new(),
            graph_scroll: ScrollHandle::new(),
            layout_dialog_scroll: ScrollHandle::new(),
        };

        // Component.onCompleted: pending folder setup opens the layout dialog.
        let pending = this.backend.pending_projects_folder_setup().to_string();
        if !pending.is_empty() {
            this.open_layout_dialog_for_folder(&pending);
        }

        this
    }

    // ---- Backend event pump ---------------------------------------------

    fn pump(&mut self, cx: &mut Context<Self>) {
        self.backend.process_pending();
        if !self.backend.has_pending_events() {
            return;
        }
        let events = self.backend.take_events();
        for event in events {
            match event {
                BackendEvent::ActivityChanged => {
                    // Main.qml scrolls the activity list to the bottom.
                    let count = self.backend.activity().len();
                    if count > 0 {
                        self.activity_scroll.scroll_to_bottom();
                    }
                }
                BackendEvent::PendingProjectsFolderSetupChanged => {
                    let pending = self.backend.pending_projects_folder_setup().to_string();
                    if !pending.is_empty() {
                        self.open_layout_dialog_for_folder(&pending);
                    }
                }
                BackendEvent::ProjectsFolderScanFinished(found_projects) => {
                    if self.layout_dialog.open && self.layout_dialog.awaiting_scan {
                        self.layout_dialog.awaiting_scan = false;
                        if found_projects {
                            self.layout_dialog.scan_succeeded = true;
                            self.layout_dialog.scan_error =
                                "Folder layout updated and projects reloaded.".to_string();
                        } else {
                            self.layout_dialog.scan_succeeded = false;
                            self.layout_dialog.scan_error = "No supported project files were found for this layout. Choose the other layout or verify the selected folder.".to_string();
                        }
                    }
                }
                BackendEvent::ThemeHueChanged => {
                    self.recompute_theme(cx);
                }
                BackendEvent::ColorSchemeModeChanged => {
                    self.color_scheme = self.backend.color_scheme_mode();
                    self.recompute_theme(cx);
                }
                BackendEvent::SelectedProjectVersionGraphChanged => {
                    if self.vm_open {
                        self.refresh_version_graph(false, cx);
                    }
                }
                BackendEvent::SelectedProjectNoteChanged => {
                    if self.vm_open {
                        let note = self.backend.selected_project_note().to_string();
                        self.project_note_input
                            .update(cx, |input, cx| input.set_text(&note, cx));
                    }
                }
                BackendEvent::ConfigReset => {
                    self.settings_open = false;
                    self.vm_open = false;
                    self.confirm = None;
                }
                BackendEvent::ProjectSaveRecorded { .. } => {
                    // OS notifications are wired up in the platform layer.
                }
                _ => {}
            }
        }
        cx.notify();
    }

    // ---- Theme -----------------------------------------------------------

    pub fn recompute_theme(&mut self, cx: &mut Context<Self>) {
        let dark = match self.color_scheme {
            ColorSchemeMode::Dark => true,
            ColorSchemeMode::Light => false,
            ColorSchemeMode::System => self.system_dark,
        };
        self.theme = Theme::compute(self.backend.theme_hue(), dark);
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
        cx.notify();
    }

    // ---- Modal helpers ----------------------------------------------------

    pub fn open_layout_dialog_for_folder(&mut self, folder: &str) {
        // ProjectsFolderLayoutDialog.openForFolder + onOpened auto-confirm.
        let local = self.backend.display_local_path(folder);
        self.layout_dialog.folder_path = local.clone();
        self.layout_dialog.awaiting_scan = false;
        self.layout_dialog.scan_error = String::new();
        self.layout_dialog.scan_succeeded = false;
        self.layout_dialog.selected_layout = self
            .backend
            .projects_folder_layout_for_path(&local)
            .to_string();
        self.layout_dialog.open = true;
        self.layout_dialog_confirm();
    }

    pub fn open_layout_dialog_for_current_folder(&mut self) {
        if self.backend.projects_folder_path().is_empty() {
            return;
        }
        self.layout_dialog.folder_path = self.backend.projects_folder_path().to_string();
        self.layout_dialog.awaiting_scan = false;
        self.layout_dialog.scan_error = String::new();
        self.layout_dialog.scan_succeeded = false;
        self.layout_dialog.selected_layout = self.backend.projects_folder_layout().to_string();
        self.layout_dialog.open = true;
    }

    pub fn layout_dialog_confirm(&mut self) {
        let local_path = self.layout_dialog.folder_path.clone();
        if local_path.is_empty() {
            self.layout_dialog.scan_error = "The selected folder path is invalid.".to_string();
            return;
        }
        self.layout_dialog.scan_error = String::new();
        self.layout_dialog.scan_succeeded = false;
        self.layout_dialog.awaiting_scan = true;
        let layout = self.layout_dialog.selected_layout.clone();
        self.backend.confirm_projects_folder(&local_path, &layout);
    }

    pub fn layout_dialog_select_and_scan(&mut self, layout: &str) {
        if self.layout_dialog.awaiting_scan {
            return;
        }
        self.layout_dialog.selected_layout = layout.to_string();
        self.layout_dialog_confirm();
    }

    pub fn open_version_manager(&mut self, visible_index: i32, cx: &mut Context<Self>) {
        self.backend.manage_project_versions(visible_index);
        self.refresh_version_graph(true, cx);
        let note = self.backend.selected_project_note().to_string();
        self.project_note_input
            .update(cx, |input, cx| input.set_text(&note, cx));
        self.vm_open = true;
        cx.notify();
    }

    pub fn refresh_version_graph(&mut self, select_latest: bool, cx: &mut Context<Self>) {
        self.vm_graph = self.backend.selected_project_version_graph();

        // applyVersionSelection
        if self.vm_graph.is_empty() {
            self.vm_selected_id = String::new();
            self.version_note_input
                .update(cx, |input, cx| input.set_text("", cx));
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
            self.version_note_input
                .update(cx, |input, cx| input.set_text(&note, cx));
        }

        // Graph extent (VersionManagerWindow.refreshVersionGraph).
        if self.vm_graph.is_empty() {
            self.vm_graph_extent = (0.0, 0.0);
        } else {
            let mut min_x = self.vm_graph[0].x;
            let mut max_x = min_x;
            let mut min_y = self.vm_graph[0].y;
            let mut max_y = min_y;
            for node in &self.vm_graph[1..] {
                min_x = min_x.min(node.x);
                max_x = max_x.max(node.x);
                min_y = min_y.min(node.y);
                max_y = max_y.max(node.y);
            }
            let node_half_w = 38.0;
            let node_half_h = 23.0;
            let label_pad = 28.0;
            let viewport_pad = 32.0;
            self.vm_graph_extent = (
                (max_x - min_x) + (node_half_w + viewport_pad) * 2.0,
                (max_y - min_y) + (node_half_h + label_pad + viewport_pad) * 2.0,
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
        self.settings_open || self.vm_open || self.layout_dialog.open
    }

    pub fn child_dialog_open(&self) -> bool {
        self.confirm.is_some() || (self.settings_open && self.layout_dialog.open)
    }

    fn close_modal(&mut self, _: &CloseModal, _window: &mut Window, cx: &mut Context<Self>) {
        if self.open_combo.is_some() {
            self.open_combo = None;
        } else if self.confirm.is_some() {
            self.confirm = None;
        } else if self.layout_dialog.open {
            self.layout_dialog.open = false;
        } else if self.vm_open {
            self.vm_open = false;
        } else if self.settings_open {
            self.settings_open = false;
        }
        cx.notify();
    }

    pub fn run_confirm_action(&mut self, cx: &mut Context<Self>) {
        if let Some(confirm) = self.confirm.take() {
            match confirm.action {
                ConfirmAction::ResetConfig => {
                    self.backend.reset_config();
                }
                ConfirmAction::DeleteVersion(version_id) => {
                    if self.backend.delete_version_by_id(&version_id) {
                        self.refresh_version_graph(false, cx);
                    }
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

        let theme = self.theme;
        let onboarding = !self.backend.has_projects_folder();

        div()
            .id("root")
            .size_full()
            .font_family(".SystemUIFont")
            .bg(theme.app_background)
            .text_color(theme.text_primary)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::close_modal))
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _window, cx| {
                let mut dirty = false;
                if this.hue_dragging {
                    this.update_hue_from_mouse(event.position, cx);
                    dirty = true;
                }
                if this.split_dragging {
                    this.update_split_from_mouse(event.position);
                    dirty = true;
                }
                if dirty {
                    cx.notify();
                }
            }))
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, _event, _window, cx| {
                    if this.hue_dragging {
                        this.hue_dragging = false;
                        this.backend.flush_pending_theme_hue_persist();
                        cx.notify();
                    }
                    if this.split_dragging {
                        this.split_dragging = false;
                        cx.notify();
                    }
                }),
            )
            .child(if onboarding {
                self.render_onboarding(cx).into_any_element()
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
                let value = (ratio * 360.0).round() as f64;
                self.backend.set_theme_hue(value);
                self.recompute_theme(cx);
            }
        }
    }

    fn update_split_from_mouse(&mut self, position: gpui::Point<Pixels>) {
        if let Some(bounds) = self.split_bounds {
            let bottom = f32::from(bounds.origin.y) + f32::from(bounds.size.height);
            // 10px handle center offset, clamp to sane panel sizes.
            let height = (bottom - f32::from(position.y) - 10.0)
                .clamp(60.0, f32::from(bounds.size.height) - 120.0);
            self.activity_panel_height = height;
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
