// Theme chosen in Settings (core::settings::Theme). "system" leaves the
// colors to prefers-color-scheme; the others force them (styles.css).
export type Theme = "system" | "light" | "dark";

/** Sent to an open viewer when the theme changes (app::settings). */
export const THEME_EVENT = "rightpeek://theme";

export function applyTheme(theme: Theme): void {
  if (theme === "system") {
    delete document.documentElement.dataset.theme;
  } else {
    document.documentElement.dataset.theme = theme;
  }
}
