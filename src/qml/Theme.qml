import QtQuick 2.15

QtObject {
    // Base hue in degrees (0-360). Adjust this for the whole theme.
    // Example: 280 = mauve-ish, 220 = blue-ish, 140 = mint-ish.
    property real hue: 280.0

    readonly property SystemPalette systemPalette: SystemPalette {
        colorGroup: SystemPalette.Active
    }

    readonly property bool isDarkMode: relativeLuminance(systemPalette.window) < relativeLuminance(systemPalette.windowText)

    function clamp01(v) {
        return Math.max(0, Math.min(1, v));
    }

    function clamp(v, minV, maxV) {
        return Math.max(minV, Math.min(maxV, v));
    }

    function srgbToLinear(v) {
        if (v <= 0.04045)
            return v / 12.92;
        return Math.pow((v + 0.055) / 1.055, 2.4);
    }

    function linearToSrgb(v) {
        const c = clamp01(v);
        if (c <= 0.0031308)
            return 12.92 * c;
        return 1.055 * Math.pow(c, 1.0 / 2.4) - 0.055;
    }

    function relativeLuminance(colorValue) {
        const r = srgbToLinear(colorValue.r);
        const g = srgbToLinear(colorValue.g);
        const b = srgbToLinear(colorValue.b);
        return 0.2126 * r + 0.7152 * g + 0.0722 * b;
    }

    function oklchToColor(L, C, hDegrees) {
        const l = clamp01(L);
        const c = Math.max(0, C);
        const hr = (hDegrees % 360.0) * Math.PI / 180.0;

        const a_ = c * Math.cos(hr);
        const b_ = c * Math.sin(hr);

        const l_ = l + 0.3963377774 * a_ + 0.2158037573 * b_;
        const m_ = l - 0.1055613458 * a_ - 0.0638541728 * b_;
        const s_ = l - 0.0894841775 * a_ - 1.2914855480 * b_;

        const l3 = l_ * l_ * l_;
        const m3 = m_ * m_ * m_;
        const s3 = s_ * s_ * s_;

        const rLin = 4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3;
        const gLin = -1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3;
        const bLin = -0.0041960863 * l3 - 0.7034186147 * m3 + 1.7076147010 * s3;

        const r = linearToSrgb(rLin);
        const g = linearToSrgb(gLin);
        const b = linearToSrgb(bLin);
        return Qt.rgba(r, g, b, 1.0);
    }

    // Pastel helper: keeps chroma in a soft range similar to Catppuccin-like palettes.
    function pastel(L, C, dH) {
        return oklchToColor(L, clamp(C, 0.0, 0.16), hue + dH);
    }

    readonly property color appBackground: isDarkMode ? pastel(0.23, 0.015, -4.0) : pastel(0.97, 0.010, -4.0)
    readonly property color panelSurface: isDarkMode ? pastel(0.27, 0.018, -2.0) : pastel(0.93, 0.012, -2.0)
    readonly property color panelSurfaceAlt: isDarkMode ? pastel(0.25, 0.018, 0.0) : pastel(0.91, 0.014, 0.0)
    readonly property color graphSurface: isDarkMode ? pastel(0.21, 0.015, -2.0) : pastel(0.95, 0.010, -2.0)
    readonly property color sidePanelSurface: isDarkMode ? pastel(0.24, 0.016, -1.0) : pastel(0.94, 0.011, -1.0)

    readonly property color textPrimary: isDarkMode ? pastel(0.95, 0.010, 0.0) : pastel(0.24, 0.018, 0.0)
    readonly property color textSecondary: isDarkMode ? pastel(0.86, 0.015, 0.0) : pastel(0.36, 0.015, 0.0)
    readonly property color textMuted: isDarkMode ? pastel(0.76, 0.016, 0.0) : pastel(0.48, 0.013, 0.0)
    readonly property color textStatus: isDarkMode ? pastel(0.90, 0.012, 2.0) : pastel(0.40, 0.015, 2.0)
    readonly property color textActivity: isDarkMode ? pastel(0.88, 0.014, 1.0) : pastel(0.42, 0.016, 1.0)
    readonly property color textMeta: isDarkMode ? pastel(0.84, 0.012, 0.0) : pastel(0.46, 0.014, 0.0)

    readonly property color accent: isDarkMode ? pastel(0.78, 0.105, 0.0) : pastel(0.62, 0.105, 0.0)
    readonly property color selection: isDarkMode ? pastel(0.84, 0.125, 12.0) : pastel(0.68, 0.125, 12.0)
    readonly property color success: oklchToColor(0.67, 0.16, 145.0)
    readonly property color successStrong: oklchToColor(0.82, 0.19, 145.0)
    readonly property color successBorder: oklchToColor(0.92, 0.08, 145.0)

    readonly property color rowEven: isDarkMode ? pastel(0.31, 0.025, -2.0) : pastel(0.90, 0.018, -2.0)
    readonly property color rowOdd: isDarkMode ? pastel(0.28, 0.025, -2.0) : pastel(0.87, 0.018, -2.0)

    readonly property color border: isDarkMode ? pastel(0.38, 0.022, 0.0) : pastel(0.78, 0.020, 0.0)
    readonly property color nodeBorder: isDarkMode ? pastel(0.92, 0.015, 0.0) : pastel(0.72, 0.016, 0.0)
    readonly property color graphLink: isDarkMode ? pastel(0.72, 0.045, 2.0) : pastel(0.58, 0.045, 2.0)
}
