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

    // Action buttons: darker fills in dark mode so icons/text stay readable.
    readonly property color buttonFill: isDarkMode ? oklchToColor(0.36, 0.06, hue) : accent
    readonly property color buttonFillHover: isDarkMode ? oklchToColor(0.44, 0.08, hue + 4.0) : selection
    readonly property color buttonLabel: isDarkMode ? pastel(0.97, 0.006, 0.0) : "#ffffff"
    readonly property color buttonIcon: buttonLabel

    readonly property color success: oklchToColor(0.67, 0.16, 145.0)
    readonly property color successStrong: oklchToColor(0.82, 0.19, 145.0)
    readonly property color successBorder: oklchToColor(0.92, 0.08, 145.0)

    readonly property color rowEven: isDarkMode ? pastel(0.31, 0.025, -2.0) : pastel(0.90, 0.018, -2.0)
    readonly property color rowOdd: isDarkMode ? pastel(0.28, 0.025, -2.0) : pastel(0.87, 0.018, -2.0)

    readonly property color border: isDarkMode ? pastel(0.38, 0.022, 0.0) : pastel(0.78, 0.020, 0.0)
    readonly property color inputSurface: isDarkMode ? pastel(0.20, 0.014, -2.0) : "#ffffff"
    readonly property color inputFill: isDarkMode ? oklchToColor(0.22, 0.045, hue) : pastel(0.96, 0.040, 0.0)
    readonly property color inputBorder: isDarkMode ? pastel(0.42, 0.020, 0.0) : pastel(0.82, 0.018, 0.0)
    readonly property color inputBorderAccent: isDarkMode ? oklchToColor(0.52, 0.085, hue) : pastel(0.68, 0.090, 0.0)
    readonly property color modalScrim: isDarkMode ? "#80000000" : "#55000000"
    readonly property color nodeBorder: isDarkMode ? pastel(0.92, 0.015, 0.0) : pastel(0.72, 0.016, 0.0)
    readonly property color graphLink: isDarkMode ? pastel(0.72, 0.045, 2.0) : pastel(0.58, 0.045, 2.0)

    // Version manager: extra separation in light mode (panels were too same-y grey).
    readonly property color vmPanel: isDarkMode ? panelSurfaceAlt : "#ffffff"
    readonly property color vmGraph: isDarkMode ? graphSurface : pastel(0.975, 0.014, -4.0)
    readonly property color vmSidePanel: isDarkMode ? sidePanelSurface : pastel(0.945, 0.028, 4.0)
    readonly property color vmBorder: isDarkMode ? border : pastel(0.68, 0.038, 0.0)
    readonly property color vmTextPrimary: isDarkMode ? textPrimary : pastel(0.18, 0.022, 0.0)
    readonly property color vmTextMeta: isDarkMode ? textMeta : pastel(0.34, 0.020, 0.0)
    readonly property color vmInputSurface: isDarkMode ? inputSurface : "#ffffff"
    readonly property color vmGraphLink: isDarkMode ? graphLink : pastel(0.52, 0.058, 2.0)
    readonly property color vmGraphLinkActive: isDarkMode ? selection : pastel(0.58, 0.095, 0.0)
    readonly property color vmGraphGrid: isDarkMode ? "#18ffffff" : "#12000000"
    readonly property color vmNodeFill: isDarkMode ? oklchToColor(0.34, 0.055, hue) : "#ffffff"
    readonly property color vmNodeSelectedFill: isDarkMode ? selection : pastel(0.66, 0.105, 0.0)
    readonly property color vmNodeCurrentFill: isDarkMode ? success : pastel(0.62, 0.120, 145.0)
    readonly property color vmNodeSelectedBorder: isDarkMode ? pastel(0.92, 0.020, 12.0) : pastel(0.52, 0.100, 0.0)
    readonly property color vmNodeLabel: isDarkMode ? textPrimary : pastel(0.22, 0.030, 0.0)
    readonly property color vmNodeShadow: isDarkMode ? "#66000000" : "#22000000"

    // Panel buttons: tinted fills, not flat grey.
    readonly property color buttonSoftFill: isDarkMode ? oklchToColor(0.36, 0.06, hue) : pastel(0.90, 0.048, 0.0)
    readonly property color buttonSoftFillHover: isDarkMode ? oklchToColor(0.44, 0.08, hue + 4.0) : pastel(0.84, 0.058, 0.0)
    readonly property color buttonSoftLabel: isDarkMode ? buttonLabel : pastel(0.26, 0.040, 0.0)
    readonly property color buttonSoftBorder: isDarkMode ? "transparent" : pastel(0.72, 0.050, 0.0)

    readonly property color buttonPrimaryFill: isDarkMode ? buttonFill : pastel(0.70, 0.095, 0.0)
    readonly property color buttonPrimaryFillHover: isDarkMode ? buttonFillHover : pastel(0.64, 0.105, 0.0)
    readonly property color buttonPrimaryLabel: isDarkMode ? buttonLabel : "#ffffff"
    readonly property color buttonPrimaryBorder: isDarkMode ? "transparent" : pastel(0.58, 0.090, 0.0)

    readonly property color buttonDangerFill: isDarkMode ? oklchToColor(0.38, 0.07, 18.0) : pastel(0.91, 0.042, 16.0)
    readonly property color buttonDangerFillHover: isDarkMode ? oklchToColor(0.44, 0.09, 18.0) : pastel(0.86, 0.052, 16.0)
    readonly property color buttonDangerLabel: isDarkMode ? pastel(0.97, 0.006, 0.0) : pastel(0.42, 0.085, 16.0)
    readonly property color buttonDangerBorder: isDarkMode ? "transparent" : pastel(0.74, 0.055, 16.0)
}
