// Viewer entry point. Rendering logic arrives in Phase 2 (docs/PLAN.md §5).
const viewer = document.querySelector<HTMLElement>("#viewer");

if (viewer) {
  viewer.textContent = "";
}
