import QtQuick 2.15

QtObject {
    // Put any seed color here (prefer hex like #7a0000 or #101010).
    // All derived tokens update automatically via OKLCH math.
    readonly property string seedHex: '#4e8200'
    readonly property var seedOklch: hexToOklch(seedHex)

    function clamp01(v) {
        return Math.max(0, Math.min(1, v))
    }

    function clamp(v, minV, maxV) {
        return Math.max(minV, Math.min(maxV, v))
    }

    function cbrt(v) {
        if (v === 0)
            return 0
        return (v < 0 ? -1 : 1) * Math.pow(Math.abs(v), 1.0 / 3.0)
    }

    function srgbToLinear(v) {
        if (v <= 0.04045)
            return v / 12.92
        return Math.pow((v + 0.055) / 1.055, 2.4)
    }

    function linearToSrgb(v) {
        const c = clamp01(v)
        if (c <= 0.0031308)
            return 12.92 * c
        return 1.055 * Math.pow(c, 1.0 / 2.4) - 0.055
    }

    function parseHex(hex) {
        let s = (hex || "").toString().trim()
        if (s.length === 0)
            s = "#000000"
        if (s[0] === "#")
            s = s.slice(1)
        if (s.length === 3) {
            s = s[0] + s[0] + s[1] + s[1] + s[2] + s[2]
        }
        if (s.length !== 6)
            return { r: 0, g: 0, b: 0 }

        const r = parseInt(s.slice(0, 2), 16) / 255.0
        const g = parseInt(s.slice(2, 4), 16) / 255.0
        const b = parseInt(s.slice(4, 6), 16) / 255.0
        return { r: r, g: g, b: b }
    }

    function hexToOklch(hex) {
        const rgb = parseHex(hex)
        const r = srgbToLinear(rgb.r)
        const g = srgbToLinear(rgb.g)
        const b = srgbToLinear(rgb.b)

        const l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b
        const m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b
        const s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b

        const l_ = cbrt(l)
        const m_ = cbrt(m)
        const s_ = cbrt(s)

        const L = 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_
        const a = 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_
        const bb = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_

        const C = Math.sqrt(a * a + bb * bb)
        let h = Math.atan2(bb, a) * 180.0 / Math.PI
        if (h < 0)
            h += 360.0

        return { l: clamp01(L), c: Math.max(0, C), h: h }
    }

    function oklchToColor(L, C, hDegrees) {
        const l = clamp01(L)
        const c = Math.max(0, C)
        const hr = (hDegrees % 360.0) * Math.PI / 180.0

        const a_ = c * Math.cos(hr)
        const b_ = c * Math.sin(hr)

        const l_ = l + 0.3963377774 * a_ + 0.2158037573 * b_
        const m_ = l - 0.1055613458 * a_ - 0.0638541728 * b_
        const s_ = l - 0.0894841775 * a_ - 1.2914855480 * b_

        const l3 = l_ * l_ * l_
        const m3 = m_ * m_ * m_
        const s3 = s_ * s_ * s_

        const rLin = 4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3
        const gLin = -1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3
        const bLin = -0.0041960863 * l3 - 0.7034186147 * m3 + 1.7076147010 * s3

        const r = linearToSrgb(rLin)
        const g = linearToSrgb(gLin)
        const b = linearToSrgb(bLin)
        return Qt.rgba(r, g, b, 1.0)
    }

    // Build tones from seed with optional L/C/H adjustments.
    function tone(dL, cScale, dH) {
        return oklchToColor(
                    clamp(seedOklch.l + dL, 0.0, 1.0),
                    clamp(seedOklch.c * cScale, 0.0, 0.37),
                    seedOklch.h + dH)
    }

    readonly property color appBackground: oklchToColor(0.27, 0.02, seedOklch.h)
    readonly property color panelSurface: oklchToColor(0.31, 0.02, seedOklch.h)
    readonly property color panelSurfaceAlt: oklchToColor(0.29, 0.02, seedOklch.h)
    readonly property color graphSurface: oklchToColor(0.23, 0.02, seedOklch.h)
    readonly property color sidePanelSurface: oklchToColor(0.25, 0.02, seedOklch.h)

    readonly property color textPrimary: oklchToColor(0.97, 0.01, seedOklch.h)
    readonly property color textSecondary: oklchToColor(0.86, 0.02, seedOklch.h)
    readonly property color textMuted: oklchToColor(0.74, 0.02, seedOklch.h)
    readonly property color textStatus: oklchToColor(0.90, 0.015, seedOklch.h)
    readonly property color textActivity: oklchToColor(0.88, 0.02, seedOklch.h)
    readonly property color textMeta: oklchToColor(0.84, 0.02, seedOklch.h)

    readonly property color accent: oklchToColor(seedOklch.l, Math.max(seedOklch.c, 0.11), seedOklch.h)
    readonly property color selection: oklchToColor(clamp(seedOklch.l + 0.10, 0.0, 1.0), Math.max(seedOklch.c, 0.12), seedOklch.h + 35.0)
    readonly property color success: oklchToColor(0.67, 0.16, 145.0)
    readonly property color successStrong: oklchToColor(0.82, 0.19, 145.0)
    readonly property color successBorder: oklchToColor(0.92, 0.08, 145.0)

    readonly property color rowEven: oklchToColor(0.36, 0.03, seedOklch.h)
    readonly property color rowOdd: oklchToColor(0.33, 0.03, seedOklch.h)

    readonly property color border: oklchToColor(0.40, 0.025, seedOklch.h)
    readonly property color nodeBorder: oklchToColor(0.93, 0.02, seedOklch.h)
    readonly property color graphLink: oklchToColor(0.66, 0.05, seedOklch.h)
}
