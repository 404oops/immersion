//! OKLCH-based theme palette.
//!
//! Every colour is derived from a single base hue in degrees, a saturation
//! multiplier on chroma (0 is greyscale, 1 the standard palette, 2 vivid)
//! and a light/dark flag. The maths lives in the vampir toolkit; this is the
//! app's own, larger set of roles built on top of it, and the mapping down
//! to the twenty-odd colours the toolkit's controls actually paint.

use gpui::Rgba;

pub use vampir::color::{argb, mix, oklch_to_color, with_alpha};
pub use vampir::{DEFAULT_SATURATION, MAX_SATURATION};

const WHITE: Rgba = vampir::color::WHITE;
const TRANSPARENT: Rgba = vampir::color::TRANSPARENT;

fn clamp(v: f64, min_v: f64, max_v: f64) -> f64 {
    v.clamp(min_v, max_v)
}

/// Saturation as the palette accepts it: clamped to the toolkit's range,
/// with a NaN (a hand-edited config) falling back to the default.
pub fn clamp_saturation(saturation: f64) -> f64 {
    if saturation.is_nan() {
        DEFAULT_SATURATION
    } else {
        saturation.clamp(0.0, MAX_SATURATION)
    }
}

/// Modal animation — opacity fade only (GPUI has no transform scale). The
/// durations live in `vampir::easing`, which drives the fades.
pub const MODAL_PANEL_RADIUS: f32 = 10.0;
pub const MODAL_EDGE_PADDING: f32 = 48.0;

/// Monospace face for identifiers, paths and timestamps: the system's own
/// mono where there is one, so it sits naturally beside the UI font.
pub const MONO_FONT: &str = if cfg!(target_os = "macos") {
    "Menlo"
} else if cfg!(target_os = "windows") {
    "Consolas"
} else {
    "DejaVu Sans Mono"
};

/// Fully evaluated theme palette; recompute when hue, saturation or dark
/// mode changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub hue: f64,
    /// Chroma multiplier, `0.0..=MAX_SATURATION`; `DEFAULT_SATURATION` is
    /// the palette as originally designed.
    pub saturation: f64,
    pub is_dark_mode: bool,

    pub app_background: Rgba,
    pub panel_surface: Rgba,
    pub panel_surface_alt: Rgba,
    pub graph_surface: Rgba,
    pub side_panel_surface: Rgba,

    pub text_primary: Rgba,
    pub text_secondary: Rgba,
    pub text_muted: Rgba,
    pub text_status: Rgba,
    pub text_activity: Rgba,
    pub text_meta: Rgba,

    pub accent: Rgba,
    pub selection: Rgba,

    pub button_fill: Rgba,
    pub button_fill_hover: Rgba,
    pub button_label: Rgba,
    pub button_icon: Rgba,

    pub success: Rgba,
    pub success_strong: Rgba,
    pub success_border: Rgba,

    pub row_even: Rgba,
    pub row_odd: Rgba,

    pub border: Rgba,
    pub input_surface: Rgba,
    pub input_fill: Rgba,
    pub input_border: Rgba,
    pub input_border_accent: Rgba,
    pub modal_scrim: Rgba,
    pub modal_border: Rgba,
    pub modal_option_border: Rgba,
    pub modal_option_fill: Rgba,
    pub modal_option_fill_hover: Rgba,
    pub modal_option_selected_fill: Rgba,
    pub modal_selected_border: Rgba,
    pub node_border: Rgba,
    pub graph_link: Rgba,

    pub vm_panel: Rgba,
    pub vm_graph: Rgba,
    pub vm_side_panel: Rgba,
    pub vm_border: Rgba,
    pub vm_text_primary: Rgba,
    pub vm_text_meta: Rgba,
    pub vm_input_surface: Rgba,
    pub vm_graph_link: Rgba,
    pub vm_graph_link_active: Rgba,
    pub vm_graph_grid: Rgba,
    pub vm_node_fill: Rgba,
    pub vm_node_selected_fill: Rgba,
    pub vm_node_current_fill: Rgba,
    pub vm_node_selected_border: Rgba,
    pub vm_node_label: Rgba,
    pub vm_node_shadow: Rgba,

    pub button_soft_fill: Rgba,
    pub button_soft_fill_hover: Rgba,
    pub button_soft_label: Rgba,
    pub button_soft_border: Rgba,

    pub button_primary_fill: Rgba,
    pub button_primary_fill_hover: Rgba,
    pub button_primary_label: Rgba,
    pub button_primary_border: Rgba,

    pub button_danger_fill: Rgba,
    pub button_danger_fill_hover: Rgba,
    pub button_danger_label: Rgba,
    pub button_danger_border: Rgba,
}

impl Theme {
    pub fn compute(hue: f64, saturation: f64, dark: bool) -> Self {
        let saturation = clamp_saturation(saturation);
        // Every chroma in the palette is scaled by the saturation, the fixed
        // hues (success green, danger red) included, so a greyscale theme is
        // grey throughout, as the toolkit's own palette is.
        let oklch_to_color = |l: f64, c: f64, h: f64| oklch_to_color(l, c * saturation, h);
        // Pastel helper: keeps chroma in a soft range (capped at 0.16).
        let pastel = |l: f64, c: f64, dh: f64| oklch_to_color(l, clamp(c, 0.0, 0.16), hue + dh);
        let pick =
            |dark_color: Rgba, light_color: Rgba| if dark { dark_color } else { light_color };

        let accent = pick(pastel(0.78, 0.105, 0.0), pastel(0.62, 0.105, 0.0));
        let selection = pick(pastel(0.84, 0.125, 12.0), pastel(0.68, 0.125, 12.0));
        let button_fill = if dark {
            oklch_to_color(0.36, 0.06, hue)
        } else {
            accent
        };
        let button_fill_hover = if dark {
            oklch_to_color(0.44, 0.08, hue + 4.0)
        } else {
            selection
        };
        let button_label = pick(pastel(0.97, 0.006, 0.0), WHITE);
        let border = pick(pastel(0.33, 0.016, 0.0), pastel(0.86, 0.014, 0.0));
        let text_primary = pick(pastel(0.95, 0.010, 0.0), pastel(0.24, 0.018, 0.0));
        let success = oklch_to_color(0.67, 0.16, 145.0);
        let graph_link = pick(pastel(0.76, 0.075, 2.0), pastel(0.54, 0.065, 2.0));
        let button_soft_fill = pick(oklch_to_color(0.36, 0.06, hue), pastel(0.90, 0.048, 0.0));
        let button_soft_fill_hover = pick(
            oklch_to_color(0.44, 0.08, hue + 4.0),
            pastel(0.84, 0.058, 0.0),
        );

        Self {
            hue,
            saturation,
            is_dark_mode: dark,

            app_background: pick(pastel(0.23, 0.015, -4.0), pastel(0.97, 0.010, -4.0)),
            panel_surface: pick(pastel(0.27, 0.018, -2.0), pastel(0.93, 0.012, -2.0)),
            panel_surface_alt: pick(pastel(0.25, 0.018, 0.0), pastel(0.91, 0.014, 0.0)),
            graph_surface: pick(pastel(0.21, 0.015, -2.0), pastel(0.95, 0.010, -2.0)),
            side_panel_surface: pick(pastel(0.24, 0.016, -1.0), pastel(0.94, 0.011, -1.0)),

            text_primary,
            text_secondary: pick(pastel(0.86, 0.015, 0.0), pastel(0.36, 0.015, 0.0)),
            text_muted: pick(pastel(0.76, 0.016, 0.0), pastel(0.48, 0.013, 0.0)),
            text_status: pick(pastel(0.90, 0.012, 2.0), pastel(0.40, 0.015, 2.0)),
            text_activity: pick(pastel(0.88, 0.014, 1.0), pastel(0.42, 0.016, 1.0)),
            text_meta: pick(pastel(0.84, 0.012, 0.0), pastel(0.46, 0.014, 0.0)),

            accent,
            selection,

            button_fill,
            button_fill_hover,
            button_label,
            button_icon: button_label,

            success,
            success_strong: oklch_to_color(0.82, 0.19, 145.0),
            success_border: oklch_to_color(0.92, 0.08, 145.0),

            row_even: pick(pastel(0.31, 0.025, -2.0), pastel(0.90, 0.018, -2.0)),
            row_odd: pick(pastel(0.28, 0.025, -2.0), pastel(0.87, 0.018, -2.0)),

            border,
            input_surface: pick(pastel(0.20, 0.014, -2.0), WHITE),
            input_fill: pick(oklch_to_color(0.22, 0.045, hue), pastel(0.96, 0.040, 0.0)),
            input_border: pick(pastel(0.35, 0.016, 0.0), pastel(0.87, 0.014, 0.0)),
            input_border_accent: pick(oklch_to_color(0.48, 0.060, hue), pastel(0.72, 0.065, 0.0)),
            modal_scrim: pick(argb(0x8000_0000), argb(0x5500_0000)),
            modal_border: pick(argb(0x2eff_ffff), argb(0x2600_0000)),
            modal_option_border: pick(argb(0x3aff_ffff), argb(0x3000_0000)),
            modal_option_fill: pick(argb(0x12ff_ffff), argb(0x0a00_0000)),
            modal_option_fill_hover: pick(argb(0x1eff_ffff), argb(0x1400_0000)),
            // Accent wash so the picked option reads at a glance, not just by
            // its border.
            modal_option_selected_fill: pick(with_alpha(accent, 0.13), with_alpha(accent, 0.11)),
            modal_selected_border: pick(pastel(0.72, 0.070, 0.0), pastel(0.64, 0.070, 0.0)),
            node_border: pick(pastel(0.58, 0.040, 0.0), pastel(0.76, 0.035, 0.0)),
            graph_link,

            // Version manager: extra separation in light mode.
            vm_panel: pick(pastel(0.29, 0.018, 0.0), WHITE),
            vm_graph: pick(pastel(0.26, 0.020, -2.0), pastel(0.975, 0.014, -4.0)),
            vm_side_panel: pick(pastel(0.30, 0.022, 2.0), pastel(0.945, 0.028, 4.0)),
            vm_border: pick(border, pastel(0.84, 0.018, 0.0)),
            vm_text_primary: pick(text_primary, pastel(0.18, 0.022, 0.0)),
            vm_text_meta: pick(pastel(0.88, 0.014, 0.0), pastel(0.34, 0.020, 0.0)),
            vm_input_surface: pick(pastel(0.26, 0.016, -2.0), WHITE),
            vm_graph_link: pick(graph_link, pastel(0.52, 0.070, 2.0)),
            vm_graph_link_active: pick(pastel(0.84, 0.130, 8.0), pastel(0.56, 0.110, 0.0)),
            vm_graph_grid: pick(argb(0x26ff_ffff), argb(0x1600_0000)),
            vm_node_fill: pick(oklch_to_color(0.40, 0.070, hue), WHITE),
            vm_node_selected_fill: pick(selection, pastel(0.66, 0.105, 0.0)),
            vm_node_current_fill: pick(success, oklch_to_color(0.62, 0.120, 145.0)),
            vm_node_selected_border: pick(pastel(0.92, 0.020, 12.0), pastel(0.52, 0.100, 0.0)),
            vm_node_label: pick(text_primary, pastel(0.22, 0.030, 0.0)),
            vm_node_shadow: pick(argb(0x6600_0000), argb(0x2200_0000)),

            // Panel buttons: tinted fills, not flat grey.
            button_soft_fill,
            button_soft_fill_hover,
            button_soft_label: pick(button_label, pastel(0.26, 0.040, 0.0)),
            button_soft_border: pick(TRANSPARENT, pastel(0.84, 0.026, 0.0)),

            button_primary_fill: pick(button_fill, pastel(0.70, 0.095, 0.0)),
            button_primary_fill_hover: pick(button_fill_hover, pastel(0.64, 0.105, 0.0)),
            button_primary_label: pick(button_label, WHITE),
            button_primary_border: pick(TRANSPARENT, pastel(0.70, 0.060, 0.0)),

            button_danger_fill: pick(oklch_to_color(0.38, 0.07, 18.0), pastel(0.91, 0.042, 16.0)),
            button_danger_fill_hover: pick(
                oklch_to_color(0.44, 0.09, 18.0),
                pastel(0.86, 0.052, 16.0),
            ),
            button_danger_label: pick(pastel(0.97, 0.006, 0.0), pastel(0.42, 0.085, 16.0)),
            button_danger_border: pick(TRANSPARENT, pastel(0.84, 0.030, 16.0)),
        }
    }
}

impl Theme {
    /// Cross-fades two palettes. Sweeping the hue instead would travel around
    /// the colour wheel and paint every hue in between — going from orange to
    /// blue would pass through green, which reads as the theme changing to
    /// something else mid-transition rather than settling.
    /// Palette part-way through a fade. Between two hues in the same colour
    /// scheme the palette is re-derived from the interpolated hue and
    /// saturation, so the fade travels around the colour wheel and never
    /// dips through the grey that a straight sRGB mix of, say, purple and
    /// green passes through. A scheme change has no hue path and mixes
    /// colour by colour instead.
    pub fn blend(from: &Theme, to: &Theme, t: f32) -> Theme {
        let t = t.clamp(0.0, 1.0);
        if from.is_dark_mode == to.is_dark_mode {
            let delta = (to.hue - from.hue + 540.0).rem_euclid(360.0) - 180.0;
            let hue = (from.hue + delta * t as f64).rem_euclid(360.0);
            let saturation = from.saturation + (to.saturation - from.saturation) * t as f64;
            Theme::compute(hue, saturation, to.is_dark_mode)
        } else {
            Theme::lerp(from, to, t)
        }
    }

    pub fn lerp(from: &Theme, to: &Theme, t: f32) -> Theme {
        let t = t.clamp(0.0, 1.0);
        Theme {
            // Bookkeeping only: the colours below are mixed, not derived from
            // this. Kept on the short arc so a switch mid-fade starts here.
            hue: {
                let delta = (to.hue - from.hue + 540.0).rem_euclid(360.0) - 180.0;
                (from.hue + delta * t as f64).rem_euclid(360.0)
            },
            saturation: from.saturation + (to.saturation - from.saturation) * t as f64,
            is_dark_mode: if t < 0.5 {
                from.is_dark_mode
            } else {
                to.is_dark_mode
            },
            app_background: mix(from.app_background, to.app_background, t),
            panel_surface: mix(from.panel_surface, to.panel_surface, t),
            panel_surface_alt: mix(from.panel_surface_alt, to.panel_surface_alt, t),
            graph_surface: mix(from.graph_surface, to.graph_surface, t),
            side_panel_surface: mix(from.side_panel_surface, to.side_panel_surface, t),
            text_primary: mix(from.text_primary, to.text_primary, t),
            text_secondary: mix(from.text_secondary, to.text_secondary, t),
            text_muted: mix(from.text_muted, to.text_muted, t),
            text_status: mix(from.text_status, to.text_status, t),
            text_activity: mix(from.text_activity, to.text_activity, t),
            text_meta: mix(from.text_meta, to.text_meta, t),
            accent: mix(from.accent, to.accent, t),
            selection: mix(from.selection, to.selection, t),
            button_fill: mix(from.button_fill, to.button_fill, t),
            button_fill_hover: mix(from.button_fill_hover, to.button_fill_hover, t),
            button_label: mix(from.button_label, to.button_label, t),
            button_icon: mix(from.button_icon, to.button_icon, t),
            success: mix(from.success, to.success, t),
            success_strong: mix(from.success_strong, to.success_strong, t),
            success_border: mix(from.success_border, to.success_border, t),
            row_even: mix(from.row_even, to.row_even, t),
            row_odd: mix(from.row_odd, to.row_odd, t),
            border: mix(from.border, to.border, t),
            input_surface: mix(from.input_surface, to.input_surface, t),
            input_fill: mix(from.input_fill, to.input_fill, t),
            input_border: mix(from.input_border, to.input_border, t),
            input_border_accent: mix(from.input_border_accent, to.input_border_accent, t),
            modal_scrim: mix(from.modal_scrim, to.modal_scrim, t),
            modal_border: mix(from.modal_border, to.modal_border, t),
            modal_option_border: mix(from.modal_option_border, to.modal_option_border, t),
            modal_option_fill: mix(from.modal_option_fill, to.modal_option_fill, t),
            modal_option_fill_hover: mix(
                from.modal_option_fill_hover,
                to.modal_option_fill_hover,
                t,
            ),
            modal_option_selected_fill: mix(
                from.modal_option_selected_fill,
                to.modal_option_selected_fill,
                t,
            ),
            modal_selected_border: mix(from.modal_selected_border, to.modal_selected_border, t),
            node_border: mix(from.node_border, to.node_border, t),
            graph_link: mix(from.graph_link, to.graph_link, t),
            vm_panel: mix(from.vm_panel, to.vm_panel, t),
            vm_graph: mix(from.vm_graph, to.vm_graph, t),
            vm_side_panel: mix(from.vm_side_panel, to.vm_side_panel, t),
            vm_border: mix(from.vm_border, to.vm_border, t),
            vm_text_primary: mix(from.vm_text_primary, to.vm_text_primary, t),
            vm_text_meta: mix(from.vm_text_meta, to.vm_text_meta, t),
            vm_input_surface: mix(from.vm_input_surface, to.vm_input_surface, t),
            vm_graph_link: mix(from.vm_graph_link, to.vm_graph_link, t),
            vm_graph_link_active: mix(from.vm_graph_link_active, to.vm_graph_link_active, t),
            vm_graph_grid: mix(from.vm_graph_grid, to.vm_graph_grid, t),
            vm_node_fill: mix(from.vm_node_fill, to.vm_node_fill, t),
            vm_node_selected_fill: mix(from.vm_node_selected_fill, to.vm_node_selected_fill, t),
            vm_node_current_fill: mix(from.vm_node_current_fill, to.vm_node_current_fill, t),
            vm_node_selected_border: mix(
                from.vm_node_selected_border,
                to.vm_node_selected_border,
                t,
            ),
            vm_node_label: mix(from.vm_node_label, to.vm_node_label, t),
            vm_node_shadow: mix(from.vm_node_shadow, to.vm_node_shadow, t),
            button_soft_fill: mix(from.button_soft_fill, to.button_soft_fill, t),
            button_soft_fill_hover: mix(from.button_soft_fill_hover, to.button_soft_fill_hover, t),
            button_soft_label: mix(from.button_soft_label, to.button_soft_label, t),
            button_soft_border: mix(from.button_soft_border, to.button_soft_border, t),
            button_primary_fill: mix(from.button_primary_fill, to.button_primary_fill, t),
            button_primary_fill_hover: mix(
                from.button_primary_fill_hover,
                to.button_primary_fill_hover,
                t,
            ),
            button_primary_label: mix(from.button_primary_label, to.button_primary_label, t),
            button_primary_border: mix(from.button_primary_border, to.button_primary_border, t),
            button_danger_fill: mix(from.button_danger_fill, to.button_danger_fill, t),
            button_danger_fill_hover: mix(
                from.button_danger_fill_hover,
                to.button_danger_fill_hover,
                t,
            ),
            button_danger_label: mix(from.button_danger_label, to.button_danger_label, t),
            button_danger_border: mix(from.button_danger_border, to.button_danger_border, t),
        }
    }
}

impl Theme {
    /// The subset of this palette the vampir controls paint with.
    ///
    /// Cheap enough to call per element: it is a copy of twenty-odd f32
    /// colours, next to nothing beside building the element itself.
    pub fn palette(&self) -> vampir::Palette {
        vampir::Palette {
            is_dark: self.is_dark_mode,
            accent: self.accent,
            backdrop: self.app_background,

            text_primary: self.text_primary,
            text_secondary: self.text_secondary,

            field_surface: self.input_surface,
            field_border: self.input_border,
            field_border_strong: self.input_border_accent,

            area_surface: self.vm_input_surface,
            area_border: self.vm_border,

            row_hover: self.row_odd,

            control_fill: self.button_fill,
            control_label: self.button_label,

            soft_fill: self.button_soft_fill,
            soft_fill_hover: self.button_soft_fill_hover,
            soft_label: self.button_soft_label,

            primary_fill: self.button_primary_fill,
            primary_fill_hover: self.button_primary_fill_hover,
            primary_label: self.button_primary_label,

            danger_fill: self.button_danger_fill,
            danger_fill_hover: self.button_danger_fill_hover,
            danger_label: self.button_danger_label,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vampir::color::{channels, relative_luminance};

    fn hex(c: Rgba) -> String {
        let [r, g, b, _] = channels(c);
        format!(
            "#{:02x}{:02x}{:02x}",
            (r * 255.0).round() as u8,
            (g * 255.0).round() as u8,
            (b * 255.0).round() as u8
        )
    }

    fn alpha(c: Rgba) -> f32 {
        channels(c)[3]
    }

    // Known-good OKLCH -> sRGB reference values at hue=280 (the default) —
    // guards the palette derivation against regressions.
    #[test]
    fn oklch_matches_reference() {
        let t = Theme::compute(280.0, DEFAULT_SATURATION, true);
        // pastel(0.23, 0.015, -4.0) at hue 280 => oklch(0.23, 0.015, 276)
        assert_eq!(
            hex(t.app_background),
            hex(oklch_to_color(0.23, 0.015, 276.0))
        );
        // success is hue-independent
        assert_eq!(hex(t.success), hex(oklch_to_color(0.67, 0.16, 145.0)));
        let light = Theme::compute(280.0, DEFAULT_SATURATION, false);
        assert_eq!(hex(light.input_surface), "#ffffff");
        assert!((alpha(light.modal_scrim) - 0x55 as f32 / 255.0).abs() < 1e-6);
        assert!((alpha(t.modal_scrim) - 0x80 as f32 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn saturation_scales_chroma() {
        // Zero saturation is grey in every role, fixed hues included.
        let grey = Theme::compute(280.0, 0.0, true);
        for c in [
            grey.accent,
            grey.app_background,
            grey.success,
            grey.button_danger_fill,
        ] {
            let [r, g, b, _] = channels(c);
            assert!((r - g).abs() < 1e-3 && (g - b).abs() < 1e-3, "{}", hex(c));
        }
        // Full saturation is the palette as designed.
        assert_eq!(
            hex(Theme::compute(280.0, 1.0, true).accent),
            hex(oklch_to_color(0.78, 0.105, 280.0))
        );
        // Out-of-range and NaN values are tamed rather than trusted.
        assert_eq!(Theme::compute(280.0, 9.0, true).saturation, MAX_SATURATION);
        assert_eq!(Theme::compute(280.0, -1.0, true).saturation, 0.0);
        assert_eq!(
            Theme::compute(280.0, f64::NAN, true).saturation,
            DEFAULT_SATURATION
        );
        // A blend within one scheme walks saturation linearly.
        let from = Theme::compute(280.0, 0.0, true);
        let to = Theme::compute(280.0, 2.0, true);
        assert!((Theme::blend(&from, &to, 0.5).saturation - 1.0).abs() < 1e-9);
    }

    #[test]
    fn luminance_dark_check() {
        let dark = Theme::compute(280.0, DEFAULT_SATURATION, true);
        let light = Theme::compute(280.0, DEFAULT_SATURATION, false);
        assert!(relative_luminance(dark.app_background) < relative_luminance(dark.text_primary));
        assert!(relative_luminance(light.app_background) > relative_luminance(light.text_primary));
    }
}
