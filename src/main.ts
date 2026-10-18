// Viewer page (docs/design.md, Viewer): pulls captured text from Rust and renders
// one <p> per paragraph with its own direction.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { THEME_EVENT, type Theme, applyTheme } from "./theme";

type Direction = "rtl" | "ltr" | "auto";

interface Paragraph {
  text: string;
  dir: Direction;
}

type ViewerContent =
  | { kind: "text"; paragraphs: Paragraph[]; plain: string }
  | { kind: "notice"; title: string; hint: string };

interface ViewerPayload {
  content: ViewerContent;
  fontSize: number;
  justify: boolean;
  theme: Theme;
}

const FONT_STEP = 2;
const PENDING_EVENT = "rightpeek://pending";
const DIRECTION_MODES = ["auto", "rtl", "ltr"] as const;
type DirectionMode = (typeof DIRECTION_MODES)[number];
const DIRECTION_LABELS: Record<DirectionMode, string> = {
  auto: "Auto",
  rtl: "RTL",
  ltr: "LTR",
};

const viewer = document.querySelector<HTMLElement>("#viewer")!;
const status = document.querySelector<HTMLElement>("#status")!;
const directionButton = document.querySelector<HTMLButtonElement>("#direction")!;
const justifyButton = document.querySelector<HTMLButtonElement>("#justify")!;
const editButton = document.querySelector<HTMLButtonElement>("#edit")!;

let plainText = "";
let fontSize = 18;
let directionMode: DirectionMode = "auto";
let justify = false;
let editing = false;
/** Bumped by every new capture, so a late edit result cannot replace it. */
let generation = 0;

function render(content: ViewerContent): void {
  viewer.replaceChildren();
  editButton.disabled = content.kind === "notice";
  if (content.kind === "notice") {
    plainText = "";
    const notice = document.createElement("section");
    notice.className = "notice";
    notice.dir = "ltr";
    const title = document.createElement("h2");
    title.textContent = content.title;
    notice.append(title);
    if (content.hint) {
      const hint = document.createElement("p");
      hint.textContent = content.hint;
      notice.append(hint);
    }
    viewer.append(notice);
    return;
  }
  plainText = content.plain;
  for (const paragraph of content.paragraphs) {
    const element = document.createElement("p");
    if (paragraph.text === "") {
      element.className = "gap";
    } else {
      element.textContent = paragraph.text;
      element.dataset.dir = paragraph.dir;
    }
    viewer.append(element);
  }
  applyDirection();
}

function applyDirection(): void {
  for (const element of viewer.querySelectorAll<HTMLElement>("p[data-dir]")) {
    element.dir = directionMode === "auto" ? element.dataset.dir! : directionMode;
  }
  directionButton.textContent = DIRECTION_LABELS[directionMode];
}

function applyFontSize(size: number): void {
  fontSize = size;
  document.documentElement.style.setProperty("--font-size", `${size}px`);
}

function applyJustify(enabled: boolean): void {
  justify = enabled;
  viewer.classList.toggle("justify", enabled);
  justifyButton.setAttribute("aria-pressed", String(enabled));
}

// Simple in-place editing: the rendered text becomes plain-text editable.
// Leaving edit mode sends the text back through core (normalize, paragraphs,
// direction) and renders the result. Edits stay in this window.
function setEditing(enabled: boolean): void {
  editing = enabled;
  viewer.classList.toggle("editing", enabled);
  editButton.setAttribute("aria-pressed", String(enabled));
  if (enabled) {
    viewer.contentEditable = "plaintext-only";
    viewer.spellcheck = false;
    viewer.focus();
  } else {
    viewer.removeAttribute("contenteditable");
  }
}

async function finishEditing(): Promise<void> {
  const text = editedText();
  const started = generation;
  setEditing(false);
  const content = await invoke<ViewerContent>("prepare_text", { text });
  if (started === generation) render(content);
}

// One line per paragraph element; a blank-line separator is an empty line.
function editedText(): string {
  return Array.from(viewer.childNodes, (node) =>
    node instanceof HTMLElement && node.classList.contains("gap") ? "" : textOf(node),
  ).join("\n");
}

// Enter in plain-text editing inserts "\n" (white-space: pre-wrap); a <br>
// is a line break too, unless it is the placeholder that ends a block.
function textOf(node: Node): string {
  if (node instanceof Text) return node.data;
  if (node.nodeName === "BR") return node.nextSibling ? "\n" : "";
  return Array.from(node.childNodes, textOf).join("");
}

async function refresh(): Promise<void> {
  const payload = await invoke<ViewerPayload>("take_pending_text");
  generation += 1;
  setEditing(false);
  applyTheme(payload.theme);
  applyFontSize(payload.fontSize);
  applyJustify(payload.justify);
  render(payload.content);
  window.scrollTo(0, 0);
  // Two frames: the text has been laid out and painted.
  requestAnimationFrame(() =>
    requestAnimationFrame(() => void invoke("viewer_ready")),
  );
}

async function changeFontSize(delta: number): Promise<void> {
  applyFontSize(await invoke<number>("set_font_size", { size: fontSize + delta }));
}

function flash(message: string): void {
  status.textContent = message;
  setTimeout(() => (status.textContent = ""), 1500);
}

async function copyAll(): Promise<void> {
  const text = editing ? editedText() : plainText;
  if (!text) return;
  try {
    await navigator.clipboard.writeText(text);
    flash("Copied");
  } catch {
    flash("Copy failed");
  }
}

document.querySelector("#copy")!.addEventListener("click", () => void copyAll());
editButton.addEventListener("click", () => {
  if (editing) void finishEditing();
  else setEditing(true);
});
document.querySelector("#smaller")!.addEventListener("click", () => void changeFontSize(-FONT_STEP));
document.querySelector("#larger")!.addEventListener("click", () => void changeFontSize(FONT_STEP));
document.querySelector("#settings")!.addEventListener("click", () => void invoke("open_settings"));
justifyButton.addEventListener("click", async () => {
  applyJustify(await invoke<boolean>("set_justify", { enabled: !justify }));
});
directionButton.addEventListener("click", () => {
  const next = (DIRECTION_MODES.indexOf(directionMode) + 1) % DIRECTION_MODES.length;
  directionMode = DIRECTION_MODES[next];
  applyDirection();
});

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && editing) {
    void finishEditing();
  } else if (event.key === "Escape") {
    void getCurrentWindow().close();
  } else if (event.ctrlKey && (event.key === "=" || event.key === "+")) {
    event.preventDefault();
    void changeFontSize(FONT_STEP);
  } else if (event.ctrlKey && event.key === "-") {
    event.preventDefault();
    void changeFontSize(-FONT_STEP);
  }
});

void listen(PENDING_EVENT, () => void refresh());
void listen<Theme>(THEME_EVENT, (event) => applyTheme(event.payload));
void refresh();
