// Settings page: records a hotkey combination, picks the theme and toggles
// autostart, showing Settings at start-up, and logging.
// Validation and registration happen in Rust (core::settings, app::hotkey).
import { invoke } from "@tauri-apps/api/core";
import { LogicalSize, getCurrentWindow } from "@tauri-apps/api/window";
import { type Theme, applyTheme } from "./theme";

interface SettingsView {
  /** The hotkey as shown to the user (Cmd/Option names on macOS). */
  hotkeyLabel: string;
  /** macOS Accessibility permission; null where capture needs none. */
  capturePermission: boolean | null;
  settings: {
    hotkey: string;
    autostart: boolean;
    fontSize: number;
    logging: boolean;
    showSettingsOnStart: boolean;
    theme: Theme;
  };
  hotkeyError: string | null;
  logDir: string | null;
  /** Set when global hotkeys are limited (Wayland): command for a desktop shortcut. */
  showCommand: string | null;
  /** Who holds the shortcut when it is not RightPeek (GNOME, the desktop portal). */
  hotkeyNote: string | null;
}

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
  "MetaLeft",
  "MetaRight",
]);

const hotkeyInput = document.querySelector<HTMLInputElement>("#hotkey")!;
const hotkeyMessage = document.querySelector<HTMLElement>("#hotkey-message")!;
const autostartBox = document.querySelector<HTMLInputElement>("#autostart")!;
const autostartMessage = document.querySelector<HTMLElement>("#autostart-message")!;
const themeSelect = document.querySelector<HTMLSelectElement>("#theme")!;
const loggingBox = document.querySelector<HTMLInputElement>("#logging")!;
const showOnStartBox = document.querySelector<HTMLInputElement>("#show-on-start")!;
const logDir = document.querySelector<HTMLElement>("#log-dir")!;
const showCommand = document.querySelector<HTMLElement>("#show-command")!;
const showCommandLine = document.querySelector<HTMLElement>("#show-command-line")!;
const hotkeyNote = document.querySelector<HTMLElement>("#hotkey-note")!;
const permission = document.querySelector<HTMLElement>("#permission")!;
const permissionStatus = document.querySelector<HTMLElement>("#permission-status")!;
const requestPermission = document.querySelector<HTMLButtonElement>("#request-permission")!;
const isMac = navigator.userAgent.includes("Mac");

function showMessage(element: HTMLElement, text: string, isError: boolean): void {
  element.textContent = text;
  element.classList.toggle("error", isError);
}

// Builds "Ctrl+Alt+KeyA" style text from a key event; Rust canonicalizes it.
function comboFromEvent(event: KeyboardEvent): string | null {
  if (MODIFIER_CODES.has(event.code)) return null;
  const parts: string[] = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.altKey) parts.push(isMac ? "Option" : "Alt");
  if (event.shiftKey) parts.push("Shift");
  if (event.metaKey) parts.push(isMac ? "Cmd" : "Super");
  parts.push(event.code);
  return parts.join("+");
}

hotkeyInput.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && !event.ctrlKey && !event.altKey) return;
  event.preventDefault();
  const combo = comboFromEvent(event);
  if (combo) hotkeyInput.value = combo;
});

document.querySelector("#save-hotkey")!.addEventListener("click", async () => {
  try {
    const saved = await invoke<string>("set_hotkey", { hotkey: hotkeyInput.value });
    hotkeyInput.value = saved;
    showMessage(hotkeyMessage, `Saved: ${saved}`, false);
  } catch (error) {
    showMessage(hotkeyMessage, String(error), true);
  }
});

autostartBox.addEventListener("change", async () => {
  try {
    const enabled = await invoke<boolean>("set_autostart", { enabled: autostartBox.checked });
    showMessage(autostartMessage, enabled ? "Enabled" : "Disabled", false);
  } catch (error) {
    autostartBox.checked = !autostartBox.checked;
    showMessage(autostartMessage, String(error), true);
  }
});

themeSelect.addEventListener("change", async () => {
  const theme = await invoke<Theme>("set_theme", { theme: themeSelect.value });
  themeSelect.value = theme;
  applyTheme(theme);
});

showOnStartBox.addEventListener("change", async () => {
  showOnStartBox.checked = await invoke<boolean>("set_show_settings_on_start", {
    enabled: showOnStartBox.checked,
  });
});

function showPermission(granted: boolean | null): void {
  permission.hidden = granted === null;
  permissionStatus.textContent = granted ? "Allowed" : "Not allowed";
  permissionStatus.classList.toggle("error", granted === false);
  requestPermission.hidden = granted === true;
}

requestPermission.addEventListener("click", async () => {
  showPermission(await invoke<boolean>("request_capture_permission"));
});

// The permission is granted in System Settings; refresh on coming back.
window.addEventListener("focus", async () => {
  if (permission.hidden) return;
  const view = await invoke<SettingsView>("get_settings");
  showPermission(view.capturePermission);
});

loggingBox.addEventListener("change", async () => {
  loggingBox.checked = await invoke<boolean>("set_logging", { enabled: loggingBox.checked });
});

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && document.activeElement !== hotkeyInput) {
    void getCurrentWindow().close();
  }
});

// Must match SETTINGS_WIDTH in src-tauri/src/app/settings.rs.
const WINDOW_WIDTH = 460;
const page = document.querySelector<HTMLElement>("main.settings")!;

// The window is created hidden; size it to the content (no empty band at the
// bottom), then show it. Messages that appear later grow it again.
async function fitToContent(): Promise<void> {
  const height = Math.ceil(page.getBoundingClientRect().height);
  await getCurrentWindow().setSize(new LogicalSize(WINDOW_WIDTH, height));
}

async function load(): Promise<void> {
  const view = await invoke<SettingsView>("get_settings");
  hotkeyInput.value = view.hotkeyLabel;
  showPermission(view.capturePermission);
  themeSelect.value = view.settings.theme;
  applyTheme(view.settings.theme);
  autostartBox.checked = view.settings.autostart;
  loggingBox.checked = view.settings.logging;
  showOnStartBox.checked = view.settings.showSettingsOnStart;
  if (view.logDir) logDir.textContent = `Folder: ${view.logDir}`;
  if (view.hotkeyError) showMessage(hotkeyMessage, view.hotkeyError, true);
  showCommand.hidden = !view.showCommand;
  showCommandLine.textContent = view.showCommand ?? "";
  hotkeyNote.hidden = !view.hotkeyNote;
  hotkeyNote.textContent = view.hotkeyNote ?? "";
}

async function start(): Promise<void> {
  try {
    await load();
    await fitToContent();
  } finally {
    const current = getCurrentWindow();
    await current.show();
    await current.setFocus();
  }
  new ResizeObserver(() => void fitToContent()).observe(page);
}

void start();
