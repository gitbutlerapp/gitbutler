import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

// WebKit ignores `app-region`, and Tauri's own drag regions only answer a press on the marked
// element itself, which Lite's strip never is: it sits under the panels. So the strip is found
// by where the press lands, as Electron finds it.
const interactive = [
	"button",
	"a",
	"input",
	"select",
	"textarea",
	"label",
	"summary",
	"[contenteditable]:not([contenteditable='false'])",
	"[tabindex]:not([tabindex='-1'])",
	"[role=button]",
	"[role=link]",
	"[role=menuitem]",
	"[role=tab]",
	"[role=checkbox]",
	"[role=radio]",
	"[role=switch]",
	"[role=option]",
].join(", ");

const inDragRegion = (event: MouseEvent): boolean => {
	if (event.button !== 0) return false;
	if (event.composedPath().some((node) => node instanceof Element && node.matches(interactive)))
		return false;
	const region = document.querySelector("[data-window-drag-region]");
	if (!region) return false;
	const { top, right, bottom, left } = region.getBoundingClientRect();
	return (
		event.clientX >= left && event.clientX < right && event.clientY >= top && event.clientY < bottom
	);
};

const toggleMaximize = () => invoke("plugin:window|internal_toggle_maximize");

// macOS zooms on the second click's release, and not if the pointer moved, as its title bars do.
let macDoubleClick: { x: number; y: number } | null = null;

document.addEventListener("mousedown", (event) => {
	if (!inDragRegion(event)) return;
	if (event.detail === 2 && window.lite.platform === "darwin") {
		macDoubleClick = { x: event.clientX, y: event.clientY };
		return;
	}
	if (event.detail !== 1 && event.detail !== 2) return;
	event.preventDefault();
	void (event.detail === 2 ? toggleMaximize() : getCurrentWindow().startDragging());
});

document.addEventListener("mouseup", (event) => {
	const pressedAt = macDoubleClick;
	macDoubleClick = null;
	if (
		pressedAt?.x === event.clientX &&
		pressedAt.y === event.clientY &&
		event.detail === 2 &&
		inDragRegion(event)
	)
		void toggleMaximize();
});
