# UI translations

`en.json` is the source catalog. The other catalogs are Serbian (Cyrillic),
French, Spanish, Greek, and German. Keys describe where text appears, so
translators can edit values without changing code. Keep the same keys and
named placeholders (`{count}`, for example) in each file.
The catalog test checks both conditions. Use `\n` in JSON for line breaks.

The catalogs are embedded in the executable. Users choose a language during
onboarding or in Settings; that choice is saved in the app configuration.
English is the default and the fallback for an untranslated key. Adding a
language requires another JSON file and an entry in `src/i18n.rs` to load and
select it, plus a choice in both language dropdowns.

The main window, settings, onboarding, version details, dialogs, save
notifications, and macOS menus use these catalogs. Technical identifiers,
file paths, project names, diagnostic activity logs, and backend messages keep
their original text. See the [translation guide](../../../wiki/Translations.md).
