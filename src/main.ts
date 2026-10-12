// Viewer page (docs/PLAN.md §5): pulls captured text from Rust and renders
// one <p> per paragraph with its own direction.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

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

let plainText = "";
let fontSize = 18;
let directionMode: DirectionMode = "auto";
let justify = false;

function render(content: ViewerContent): void {
  viewer.replaceChildren();
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

async function refresh(): Promise<void> {
  const payload = await invoke<ViewerPayload>("take_pending_text");
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
  if (!plainText) return;
  try {
    await navigator.clipboard.writeText(plainText);
    flash("Copied");
  } catch {
    flash("Copy failed");
  }
}

document.querySelector("#copy")!.addEventListener("click", () => void copyAll());
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
  if (event.key === "Escape") {
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
void refresh();
