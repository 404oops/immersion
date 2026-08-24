//! First-run wizard: pick a projects folder, choose its layout, watch the
//! scan run, then optionally add more folders — repeat until done.

use gpui::{
    Context, FontWeight, MouseButton, PathPromptOptions, SharedString, Window, div, prelude::*, px,
};

use crate::app::{OnboardingStep, RootView};
use crate::theme::{MODAL_PANEL_RADIUS, Theme};
use crate::ui::controls::{ButtonVariant, panel_button};

impl RootView {
    pub fn render_onboarding(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<RootView>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let step = self.onboarding.clone().unwrap_or(OnboardingStep::Welcome);

        let content = match step {
            OnboardingStep::Welcome => self.render_welcome_step(&theme, cx),
            OnboardingStep::ChooseLayout { folder } => self.render_layout_step(&theme, folder, cx),
            OnboardingStep::Scanning { folder } => self.render_scanning_step(&theme, folder),
            OnboardingStep::AddMore { found_projects } => {
                self.render_add_more_step(&theme, found_projects, cx)
            }
        };

        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(560.0))
                    .rounded(px(MODAL_PANEL_RADIUS))
                    .bg(theme.panel_surface)
                    .border_1()
                    .border_color(theme.node_border)
                    .p(px(28.0))
                    .flex()
                    .flex_col()
                    .gap(px(16.0))
                    .child(content),
            )
    }

    fn render_welcome_step(
        &mut self,
        theme: &Theme,
        cx: &mut Context<RootView>,
    ) -> gpui::AnyElement {
        div()
            .flex()
            .flex_col()
            .gap(px(14.0))
            .child(wizard_title(theme, "Welcome to Immersion"))
            .child(wizard_text(
                theme,
                "Immersion automatically versions your DAW project files (Ableton, \
                 Bitwig, FL Studio, Logic, Reaper, ...) every time you save.\n\n\
                 Start by choosing a folder that contains your projects.",
            ))
            .child(div().h(px(40.0)).w(px(260.0)).child(panel_button(
                "onboarding-choose-folder",
                "Choose Projects Folder…",
                ButtonVariant::Primary,
                true,
                theme,
                self,
                cx,
                |this, _w, cx| {
                    this.prompt_for_onboarding_folder(cx);
                },
            )))
            .into_any_element()
    }

    fn render_layout_step(
        &mut self,
        theme: &Theme,
        folder: String,
        cx: &mut Context<RootView>,
    ) -> gpui::AnyElement {
        let folder_for_bundles = folder.clone();
        let folder_for_files = folder.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(wizard_title(theme, "How is this folder organized?"))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme.text_muted)
                    .truncate()
                    .child(SharedString::from(folder.clone())),
            )
            .child(self.layout_card(
                "onboarding-layout-bundles",
                "One folder per project",
                "Each project lives in its own subfolder (an Ableton or Bitwig \
                 project folder, a Logic bundle, ...).",
                theme,
                cx,
                move |this, cx| {
                    this.backend
                        .add_projects_folder(&folder_for_bundles, "Bundles");
                    this.onboarding = Some(OnboardingStep::Scanning {
                        folder: folder_for_bundles.clone(),
                    });
                    cx.notify();
                },
            ))
            .child(self.layout_card(
                "onboarding-layout-files",
                "Loose project files",
                "Project files sit directly in the folder (a folder full of .als \
                 or .flp files); every file is its own project.",
                theme,
                cx,
                move |this, cx| {
                    this.backend.add_projects_folder(&folder_for_files, "Files");
                    this.onboarding = Some(OnboardingStep::Scanning {
                        folder: folder_for_files.clone(),
                    });
                    cx.notify();
                },
            ))
            .child(div().h(px(34.0)).w(px(120.0)).child(panel_button(
                "onboarding-layout-back",
                "Back",
                ButtonVariant::Soft,
                true,
                theme,
                self,
                cx,
                |this, _w, cx| {
                    this.onboarding = Some(OnboardingStep::Welcome);
                    cx.notify();
                },
            )))
            .into_any_element()
    }

    fn render_scanning_step(&mut self, theme: &Theme, folder: String) -> gpui::AnyElement {
        div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(wizard_title(theme, "Scanning for projects…"))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme.text_muted)
                    .truncate()
                    .child(SharedString::from(folder)),
            )
            .child(wizard_text(
                theme,
                self.backend.status_message().to_string(),
            ))
            .into_any_element()
    }

    fn render_add_more_step(
        &mut self,
        theme: &Theme,
        found_projects: bool,
        cx: &mut Context<RootView>,
    ) -> gpui::AnyElement {
        let project_count = self.backend.projects().len();
        let summary = if found_projects {
            format!(
                "Found {project_count} project{} — they are now being versioned automatically.",
                if project_count == 1 { "" } else { "s" }
            )
        } else {
            "No supported project files were found in that folder. You can try the \
             other layout, or pick a different folder."
                .to_string()
        };

        div()
            .flex()
            .flex_col()
            .gap(px(14.0))
            .child(wizard_title(
                theme,
                if found_projects {
                    "Folder added"
                } else {
                    "Nothing found"
                },
            ))
            .child(wizard_text(theme, summary))
            .child(wizard_text(
                theme,
                "Do you want to add another projects folder? You can always add or \
                 remove folders later from the tab bar.",
            ))
            .child(
                div()
                    .flex()
                    .gap(px(10.0))
                    .when(!found_projects, |el| {
                        el.child(div().h(px(38.0)).flex_1().child(panel_button(
                            "onboarding-retry-layout",
                            "Try Other Layout",
                            ButtonVariant::Soft,
                            true,
                            theme,
                            self,
                            cx,
                            |this, _w, cx| {
                                let folder = this.backend.projects_folder_path().to_string();
                                if !folder.is_empty() {
                                    this.onboarding = Some(OnboardingStep::ChooseLayout { folder });
                                }
                                cx.notify();
                            },
                        )))
                    })
                    .child(div().h(px(38.0)).flex_1().child(panel_button(
                        "onboarding-add-more",
                        "Add Another Folder…",
                        ButtonVariant::Soft,
                        true,
                        theme,
                        self,
                        cx,
                        |this, _w, cx| {
                            this.prompt_for_onboarding_folder(cx);
                        },
                    )))
                    .child(div().h(px(38.0)).flex_1().child(panel_button(
                        "onboarding-finish",
                        "Done",
                        ButtonVariant::Primary,
                        true,
                        theme,
                        self,
                        cx,
                        |this, _w, cx| {
                            if this.backend.has_projects_folder() {
                                this.onboarding = None;
                            } else {
                                this.onboarding = Some(OnboardingStep::Welcome);
                            }
                            cx.notify();
                        },
                    ))),
            )
            .into_any_element()
    }

    fn layout_card(
        &mut self,
        id: &'static str,
        title: &'static str,
        description: &'static str,
        theme: &Theme,
        cx: &mut Context<RootView>,
        on_pick: impl Fn(&mut RootView, &mut Context<RootView>) + 'static,
    ) -> impl IntoElement {
        let theme = *theme;
        div()
            .id(id)
            .w_full()
            .rounded(px(8.0))
            .bg(theme.row_odd)
            .border_1()
            .border_color(theme.node_border)
            .p(px(12.0))
            .flex()
            .flex_col()
            .gap(px(4.0))
            .cursor_pointer()
            .hover(move |style| style.border_color(theme.accent))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _event, _window, cx| {
                    on_pick(this, cx);
                }),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text_primary)
                    .child(title),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_color(theme.text_muted)
                    .child(description),
            )
    }

    /// Folder picker used by the wizard: continues to the layout step.
    pub fn prompt_for_onboarding_folder(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Select Projects Folder".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                if let Some(path) = paths.first() {
                    let folder = path.to_string_lossy().to_string();
                    this.update(cx, |root: &mut RootView, cx| {
                        let local = root.backend.display_local_path(&folder);
                        root.onboarding = Some(OnboardingStep::ChooseLayout { folder: local });
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }
}

fn wizard_title(theme: &Theme, text: &'static str) -> impl IntoElement {
    div()
        .text_size(px(21.0))
        .font_weight(FontWeight::BOLD)
        .text_color(theme.text_primary)
        .child(text)
}

fn wizard_text(theme: &Theme, text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_size(px(13.0))
        .line_height(px(18.0))
        .text_color(theme.text_secondary)
        .child(text.into())
}
