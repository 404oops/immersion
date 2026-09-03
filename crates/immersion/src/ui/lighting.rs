//! Lighting: the few gradient, highlight and shadow recipes that give
//! surfaces and controls a lit, physical feel. Everything is derived from
//! the theme's own colours, so the hue slider and dark mode keep working.

use gpui::{Background, BoxShadow, Hsla, Rgba, linear_color_stop, linear_gradient, point, px};

/// Mixes `color` toward white (`amount` > 0) or black (`amount` < 0).
pub fn shade(color: Rgba, amount: f32) -> Rgba {
    let target = if amount >= 0.0 { 1.0 } else { 0.0 };
    let t = amount.abs().clamp(0.0, 1.0);
    Rgba {
        r: color.r + (target - color.r) * t,
        g: color.g + (target - color.g) * t,
        b: color.b + (target - color.b) * t,
        a: color.a,
    }
}

fn white(alpha: f32) -> Hsla {
    Rgba {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: alpha,
    }
    .into()
}

fn black(alpha: f32) -> Hsla {
    Rgba {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: alpha,
    }
    .into()
}

/// Perceived brightness of a colour, 0..=1.
fn luminance(color: Rgba) -> f32 {
    0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b
}

/// Vertical gradient lit from above: `lift` lighter at the top and darker
/// at the bottom. Near-white fills cannot get any lighter, so the brighter
/// the base the more of the effect goes into darkening the bottom edge;
/// that keeps light mode as sculpted as dark mode.
pub fn lit(base: Rgba, lift: f32) -> Background {
    let bottom = lift * (0.5 + 0.5 * luminance(base));
    linear_gradient(
        180.0,
        linear_color_stop(shade(base, lift), 0.0),
        linear_color_stop(shade(base, -bottom), 1.0),
    )
}

/// Rim colour for a raised control: a touch darker than its fill.
pub fn rim(fill: Rgba, dark: bool) -> Rgba {
    shade(fill, if dark { -0.35 } else { -0.2 })
}

/// Raised control: a crisp highlight along the inner top edge, a soft drop
/// shadow underneath and, in light mode, a faint bevel along the inner
/// bottom edge (the highlight alone is invisible on near-white fills).
pub fn raised(dark: bool) -> Vec<BoxShadow> {
    let mut shadows = vec![
        BoxShadow {
            color: white(if dark { 0.08 } else { 0.55 }),
            offset: point(px(0.0), px(1.0)),
            blur_radius: px(0.0),
            spread_radius: px(0.0),
            inset: true,
        },
        BoxShadow {
            color: black(if dark { 0.22 } else { 0.1 }),
            offset: point(px(0.0), px(1.0)),
            blur_radius: px(2.0),
            spread_radius: px(0.0),
            inset: false,
        },
    ];
    if !dark {
        shadows.push(BoxShadow {
            color: black(0.05),
            offset: point(px(0.0), px(-1.0)),
            blur_radius: px(0.0),
            spread_radius: px(0.0),
            inset: true,
        });
    }
    shadows
}

/// Recessed well (fields, tracks): a soft shadow inside the top edge.
pub fn recessed(dark: bool) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: black(if dark { 0.3 } else { 0.1 }),
        offset: point(px(0.0), px(1.5)),
        blur_radius: px(2.5),
        spread_radius: px(0.0),
        inset: true,
    }]
}

/// Floating panel: a lit top edge and a wide, soft shadow.
pub fn panel(dark: bool) -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: white(if dark { 0.05 } else { 0.7 }),
            offset: point(px(0.0), px(1.0)),
            blur_radius: px(0.0),
            spread_radius: px(0.0),
            inset: true,
        },
        BoxShadow {
            color: black(if dark { 0.25 } else { 0.09 }),
            offset: point(px(0.0), px(2.0)),
            blur_radius: px(10.0),
            spread_radius: px(0.0),
            inset: false,
        },
    ]
}

/// The same shadows at `t` of their strength, for elements fading in or out.
pub fn faded(mut shadows: Vec<BoxShadow>, t: f32) -> Vec<BoxShadow> {
    let t = t.clamp(0.0, 1.0);
    for shadow in &mut shadows {
        shadow.color.a *= t;
    }
    shadows
}

/// Coloured glow around an element (current node, focused field).
pub fn glow(color: Rgba, alpha: f32, radius: f32) -> BoxShadow {
    BoxShadow {
        color: Rgba { a: alpha, ..color }.into(),
        offset: point(px(0.0), px(0.0)),
        blur_radius: px(radius),
        spread_radius: px(radius * 0.25),
        inset: false,
    }
}
