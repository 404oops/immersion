//! First-run wizard: pick a projects folder, choose its layout, watch the
//! scan run, then optionally add more folders — repeat until done.

use gpui::{Context, FontWeight, PathPromptOptions, SharedString, Window, div, prelude::*, px};

use crate::app::{OnboardingStep, RootView};
use crate::theme::{MODAL_PANEL_RADIUS, Theme};
use crate::ui::controls::{ButtonVariant, panel_button};
use vampir::{Choice, radio_group};

/// The two ways a projects folder can be laid out: the backend's name for
/// each, the label the user picks, and what it means. The wizard and the
/// layout dialog offer the same list.
pub const LAYOUTS: [(&str, &str, &str); 2] = [
    (
        "Bundles",
        "One folder per project",
        "Each project has its own subfolder or bundle, like .logicx or .scriv.",
    ),
    (
        "Files",
        "Loose project files",
        "Project files like .als, .blend, or .sketch sit directly in the folder.",
    ),
];

/// [`LAYOUTS`] as the choices of a radio group.
pub fn layout_choices() -> Vec<Choice> {
    LAYOUTS
        .iter()
        .map(|(_, title, detail)| Choice::new(*title).detail(*detail))
        .collect()
}

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
                "Immersion automatically versions supported creative project files \
                 (Ableton Live, Blender, Affinity, Sketch, and more) as you save.\n\n\
                 Start by choosing a folder that contains your projects.",
            ))
            .child(div().h(px(30.0)).w(px(240.0)).child(panel_button(
                "onboarding-choose-folder",
                "Choose Projects Folder…",
                ButtonVariant::Primary,
                true,
                theme,
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
        let choices = layout_choices();
        let selected = self.onboarding_layout.min(LAYOUTS.len() - 1);
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
            .child(radio_group(
                "onboarding-layout",
                &choices,
                selected,
                true,
                self.widget_context(cx),
                |this, index, _window, cx| {
                    this.onboarding_layout = index;
                    cx.notify();
                },
            ))
            .child(
                div()
                    .flex()
                    .gap(px(10.0))
                    .child(div().h(px(30.0)).w(px(120.0)).child(panel_button(
                        "onboarding-layout-back",
                        "Back",
                        ButtonVariant::Soft,
                        true,
                        theme,
                        cx,
                        |this, _w, cx| {
                            this.onboarding = Some(OnboardingStep::Welcome);
                            cx.notify();
                        },
                    )))
                    .child(div().flex_1())
                    .child(div().h(px(30.0)).w(px(140.0)).child(panel_button(
                        "onboarding-layout-continue",
                        "Continue",
                        ButtonVariant::Primary,
                        true,
                        theme,
                        cx,
                        move |this, _w, cx| {
                            let layout = LAYOUTS[this.onboarding_layout.min(LAYOUTS.len() - 1)].0;
                            this.backend.add_projects_folder(&folder, layout);
                            this.onboarding = Some(OnboardingStep::Scanning {
                                folder: folder.clone(),
                            });
                            cx.notify();
                        },
                    ))),
            )
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
                        el.child(div().h(px(30.0)).flex_1().child(panel_button(
                            "onboarding-retry-layout",
                            "Try Other Layout",
                            ButtonVariant::Soft,
                            true,
                            theme,
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
                    .child(div().h(px(30.0)).flex_1().child(panel_button(
                        "onboarding-add-more",
                        "Add Another Folder…",
                        ButtonVariant::Soft,
                        true,
                        theme,
                        cx,
                        |this, _w, cx| {
                            this.prompt_for_onboarding_folder(cx);
                        },
                    )))
                    .child(div().h(px(30.0)).flex_1().child(panel_button(
                        "onboarding-finish",
                        "Done",
                        ButtonVariant::Primary,
                        true,
                        theme,
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

    /// Folder picker used by the wizard: continues to the layout step.
    pub fn prompt_for_onboarding_folder(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Select Projects Folder".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await
                && let Some(path) = paths.first()
            {
                let folder = path.to_string_lossy().to_string();
                this.update(cx, |root: &mut RootView, cx| {
                    let local = root.backend.display_local_path(&folder);
                    root.onboarding = Some(OnboardingStep::ChooseLayout { folder: local });
                    cx.notify();
                })
                .ok();
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
