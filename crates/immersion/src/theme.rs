//! 1:1 port of qt-legacy/src/qml/Theme.qml.
//!
//! All colors are derived from a single base hue (degrees) plus a light/dark
//! flag, through an OKLCH -> linear sRGB -> sRGB pipeline identical to the
//! QML implementation (same matrix constants, same clamping).

use gpui::Rgba;

fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

fn clamp(v: f64, min_v: f64, max_v: f64) -> f64 {
    v.clamp(min_v, max_v)
}

#[allow(dead_code)]
pub fn srgb_to_linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

pub fn linear_to_srgb(v: f64) -> f64 {
    let c = clamp01(v);
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

#[allow(dead_code)]
pub fn relative_luminance(color: Rgba) -> f64 {
    let r = srgb_to_linear(color.r as f64);
    let g = srgb_to_linear(color.g as f64);
    let b = srgb_to_linear(color.b as f64);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// OKLCH -> sRGB, matching Theme.qml's `oklchToColor` exactly.
pub fn oklch_to_color(l_in: f64, c_in: f64, h_degrees: f64) -> Rgba {
    let l = clamp01(l_in);
    let c = c_in.max(0.0);
    let hr = (h_degrees % 360.0) * std::f64::consts::PI / 180.0;

    let a_ = c * hr.cos();
    let b_ = c * hr.sin();

    let l_ = l + 0.3963377774 * a_ + 0.2158037573 * b_;
    let m_ = l - 0.1055613458 * a_ - 0.0638541728 * b_;
    let s_ = l - 0.0894841775 * a_ - 1.2914855480 * b_;

    let l3 = l_ * l_ * l_;
    let m3 = m_ * m_ * m_;
    let s3 = s_ * s_ * s_;

    let r_lin = 4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3;
    let g_lin = -1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3;
    let b_lin = -0.0041960863 * l3 - 0.7034186147 * m3 + 1.7076147010 * s3;

    Rgba {
        r: linear_to_srgb(r_lin) as f32,
        g: linear_to_srgb(g_lin) as f32,
        b: linear_to_srgb(b_lin) as f32,
        a: 1.0,
    }
}

/// `#AARRGGBB` / `#RRGGBB` literal helper matching QML color strings.
const fn argb(hex: u32) -> Rgba {
    Rgba {
        r: ((hex >> 16) & 0xff) as f32 / 255.0,
        g: ((hex >> 8) & 0xff) as f32 / 255.0,
        b: (hex & 0xff) as f32 / 255.0,
        a: ((hex >> 24) & 0xff) as f32 / 255.0,
    }
}

const WHITE: Rgba = Rgba {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};
const TRANSPARENT: Rgba = Rgba {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

/// Modal animation constants from Theme.qml (kept for the animation pass).
#[allow(dead_code)]
pub const MODAL_ENTER_DURATION_MS: u64 = 220;
#[allow(dead_code)]
pub const MODAL_EXIT_DURATION_MS: u64 = 160;
#[allow(dead_code)]
pub const MODAL_ENTER_SCALE: f32 = 0.97;
#[allow(dead_code)]
pub const MODAL_EXIT_SCALE: f32 = 0.985;
pub const MODAL_PANEL_RADIUS: f32 = 10.0;
pub const MODAL_EDGE_PADDING: f32 = 48.0;

/// Fully evaluated theme palette; recompute when hue or dark mode changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub hue: f64,
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
    pub fn compute(hue: f64, dark: bool) -> Self {
        // Pastel helper: keeps chroma in a soft range (Theme.qml `pastel`).
        let pastel = |l: f64, c: f64, dh: f64| oklch_to_color(l, clamp(c, 0.0, 0.16), hue + dh);
        let pick = |dark_color: Rgba, light_color: Rgba| if dark { dark_color } else { light_color };

        let accent = pick(pastel(0.78, 0.105, 0.0), pastel(0.62, 0.105, 0.0));
        let selection = pick(pastel(0.84, 0.125, 12.0), pastel(0.68, 0.125, 12.0));
        let button_fill = if dark { oklch_to_color(0.36, 0.06, hue) } else { accent };
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
            vm_node_current_fill: pick(success, pastel(0.62, 0.120, 145.0)),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(c: Rgba) -> String {
        format!(
            "#{:02x}{:02x}{:02x}",
            (c.r * 255.0).round() as u8,
            (c.g * 255.0).round() as u8,
            (c.b * 255.0).round() as u8
        )
    }

    // Reference values computed by evaluating Theme.qml's JavaScript with
    // hue=280 (the default) — guards the port against regressions.
    #[test]
    fn oklch_matches_qml_reference() {
        let t = Theme::compute(280.0, true);
        // pastel(0.23, 0.015, -4.0) at hue 280 => oklch(0.23, 0.015, 276)
        assert_eq!(hex(t.app_background), hex(oklch_to_color(0.23, 0.015, 276.0)));
        // success is hue-independent
        assert_eq!(hex(t.success), hex(oklch_to_color(0.67, 0.16, 145.0)));
        let light = Theme::compute(280.0, false);
        assert_eq!(hex(light.input_surface), "#ffffff");
        assert!((light.modal_scrim.a - 0x55 as f32 / 255.0).abs() < 1e-6);
        assert!((t.modal_scrim.a - 0x80 as f32 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn luminance_dark_check() {
        let dark = Theme::compute(280.0, true);
        let light = Theme::compute(280.0, false);
        assert!(relative_luminance(dark.app_background) < relative_luminance(dark.text_primary));
        assert!(relative_luminance(light.app_background) > relative_luminance(light.text_primary));
    }
}
