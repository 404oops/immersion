//! The colours the controls draw with.
//!
//! [`Palette`] is deliberately small: roughly twenty colours, all of them
//! things a control actually paints. A host with its own richer theme fills
//! one in from that theme; a host without one calls [`Palette::from_hue`]
//! and gets the whole set derived from a single hue.

use gpui::Rgba;

use crate::color::{TRANSPARENT, WHITE, argb, oklch_to_color};

/// Colours for one colour scheme at one hue.
///
/// `Copy`, and cheap enough to build per frame, so a host can hand controls
/// a freshly derived palette during a theme cross-fade without caching.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    /// Drives shadow and highlight strength throughout [`crate::lighting`]:
    /// dark schemes are lit with white highlights, light ones with shadow.
    pub is_dark: bool,

    /// The one saturated colour: focus rings, the switch's on state, slider
    /// fills, an open pop-up's border.
    pub accent: Rgba,

    /// Body text, and the text inside fields.
    pub text_primary: Rgba,
    /// Captions, step glyphs, scrollbar thumbs. Quieter than primary.
    pub text_secondary: Rgba,

    /// Inside of a single-line field, and the switch knob and slider handle.
    pub field_surface: Rgba,
    /// A field's resting border, and the slider's unfilled track.
    pub field_border: Rgba,
    /// Firmer border for the small round parts that need to read against a
    /// light surface: the switch knob, the slider handle.
    pub field_border_strong: Rgba,

    /// Inside of a multi-line text area. Often the same as `field_surface`;
    /// separate because areas sit on a different panel in some layouts.
    pub area_surface: Rgba,
    /// A text area's resting border.
    pub area_border: Rgba,

    /// Hover wash behind an unselected row in a pop-up list.
    pub row_hover: Rgba,

    /// The accent-tinted control fill: the pop-up's chevron chip, and the
    /// highlighted row in its list.
    pub control_fill: Rgba,
    /// Label and chevron colour on `control_fill`.
    pub control_label: Rgba,

    /// Secondary button: tinted, not flat grey.
    pub soft_fill: Rgba,
    pub soft_fill_hover: Rgba,
    pub soft_label: Rgba,

    /// The single call to action in a group.
    pub primary_fill: Rgba,
    pub primary_fill_hover: Rgba,
    pub primary_label: Rgba,

    /// Destructive action: tinted red, still a button rather than a warning.
    pub danger_fill: Rgba,
    pub danger_fill_hover: Rgba,
    pub danger_label: Rgba,
}

impl Palette {
    /// The whole palette from one hue in degrees and a light/dark flag.
    ///
    /// Chroma stays inside a soft range, so every hue lands as a pastel of
    /// the same intensity and a hue slider never sends the UI garish.
    pub fn from_hue(hue: f64, dark: bool) -> Self {
        let pastel = |l: f64, c: f64, dh: f64| oklch_to_color(l, c.clamp(0.0, 0.16), hue + dh);
        let pick =
            |dark_color: Rgba, light_color: Rgba| if dark { dark_color } else { light_color };

        let accent = pick(pastel(0.78, 0.105, 0.0), pastel(0.62, 0.105, 0.0));
        let selection = pick(pastel(0.84, 0.125, 12.0), pastel(0.68, 0.125, 12.0));
        let control_fill = pick(oklch_to_color(0.36, 0.06, hue), accent);
        let control_fill_hover = pick(oklch_to_color(0.44, 0.08, hue + 4.0), selection);
        let control_label = pick(pastel(0.97, 0.006, 0.0), WHITE);
        let border = pick(pastel(0.33, 0.016, 0.0), pastel(0.86, 0.014, 0.0));
        let text_primary = pick(pastel(0.95, 0.010, 0.0), pastel(0.24, 0.018, 0.0));

        Self {
            is_dark: dark,
            accent,

            text_primary,
            text_secondary: pick(pastel(0.86, 0.015, 0.0), pastel(0.36, 0.015, 0.0)),

            field_surface: pick(pastel(0.20, 0.014, -2.0), WHITE),
            field_border: pick(pastel(0.35, 0.016, 0.0), pastel(0.87, 0.014, 0.0)),
            field_border_strong: pick(oklch_to_color(0.48, 0.060, hue), pastel(0.72, 0.065, 0.0)),

            area_surface: pick(pastel(0.26, 0.016, -2.0), WHITE),
            area_border: pick(border, pastel(0.84, 0.018, 0.0)),

            row_hover: pick(pastel(0.28, 0.025, -2.0), pastel(0.87, 0.018, -2.0)),

            control_fill,
            control_label,

            soft_fill: pick(oklch_to_color(0.36, 0.06, hue), pastel(0.90, 0.048, 0.0)),
            soft_fill_hover: pick(
                oklch_to_color(0.44, 0.08, hue + 4.0),
                pastel(0.84, 0.058, 0.0),
            ),
            soft_label: pick(control_label, pastel(0.26, 0.040, 0.0)),

            primary_fill: pick(control_fill, pastel(0.70, 0.095, 0.0)),
            primary_fill_hover: pick(control_fill_hover, pastel(0.64, 0.105, 0.0)),
            primary_label: pick(control_label, WHITE),

            danger_fill: pick(oklch_to_color(0.38, 0.07, 18.0), pastel(0.91, 0.042, 16.0)),
            danger_fill_hover: pick(oklch_to_color(0.44, 0.09, 18.0), pastel(0.86, 0.052, 16.0)),
            danger_label: pick(pastel(0.97, 0.006, 0.0), pastel(0.42, 0.085, 16.0)),
        }
    }

    /// Neutral grey palette for a host with no hue of its own.
    pub fn neutral(dark: bool) -> Self {
        Self::from_hue(265.0, dark)
    }

    /// A transvampir black or white wash to lay over content behind a modal.
    /// Not used by any control here; handy for the host that hosts them.
    pub fn scrim(&self) -> Rgba {
        if self.is_dark {
            argb(0x8000_0000)
        } else {
            argb(0x5500_0000)
        }
    }

    /// A fully transparent colour in this palette's space, for `when` arms
    /// that need to paint nothing.
    pub const fn transparent() -> Rgba {
        TRANSPARENT
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::neutral(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::relative_luminance;

    #[test]
    fn schemes_run_in_opposite_directions() {
        let dark = Palette::from_hue(280.0, true);
        let light = Palette::from_hue(280.0, false);
        assert!(relative_luminance(dark.field_surface) < relative_luminance(dark.text_primary));
        assert!(relative_luminance(light.field_surface) > relative_luminance(light.text_primary));
    }

    #[test]
    fn every_hue_stays_in_gamut() {
        for hue in (0..360).step_by(15) {
            for dark in [true, false] {
                let palette = Palette::from_hue(hue as f64, dark);
                for color in [
                    palette.accent,
                    palette.control_fill,
                    palette.primary_fill,
                    palette.soft_fill,
                    palette.danger_fill,
                ] {
                    for channel in [color.r, color.g, color.b, color.a] {
                        assert!(
                            (0.0..=1.0).contains(&channel),
                            "hue {hue} dark {dark} produced {channel}"
                        );
                    }
                }
            }
        }
    }
}
