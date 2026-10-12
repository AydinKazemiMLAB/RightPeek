// Settings page: records a hotkey combination and toggles autostart and
// logging.
// Validation and registration happen in Rust (core::settings, app::hotkey).
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface SettingsView {
  settings: {
    hotkey: string;
    autostart: boolean;
    fontSize: number;
    logging: boolean;
  };
  hotkeyError: string | null;
  logDir: string | null;
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
const loggingBox = document.querySelector<HTMLInputElement>("#logging")!;
const logDir = document.querySelector<HTMLElement>("#log-dir")!;

function showMessage(element: HTMLElement, text: string, isError: boolean): void {
  element.textContent = text;
  element.classList.toggle("error", isError);
}

// Builds "Ctrl+Alt+KeyA" style text from a key event; Rust canonicalizes it.
function comboFromEvent(event: KeyboardEvent): string | null {
  if (MODIFIER_CODES.has(event.code)) return null;
  const parts: string[] = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  if (event.metaKey) parts.push("Super");
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

loggingBox.addEventListener("change", async () => {
  loggingBox.checked = await invoke<boolean>("set_logging", { enabled: loggingBox.checked });
});

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && document.activeElement !== hotkeyInput) {
    void getCurrentWindow().close();
  }
});

async function load(): Promise<void> {
  const view = await invoke<SettingsView>("get_settings");
  hotkeyInput.value = view.settings.hotkey;
  autostartBox.checked = view.settings.autostart;
  loggingBox.checked = view.settings.logging;
  if (view.logDir) logDir.textContent = `Folder: ${view.logDir}`;
  if (view.hotkeyError) showMessage(hotkeyMessage, view.hotkeyError, true);
}

void load();
