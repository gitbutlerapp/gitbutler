const ESCAPE_KEYDOWN_OPTIONS = { capture: true } as const;

let activeDragCount = 0;

function handleKeyDown(event: KeyboardEvent) {
	if (event.key !== "Escape") return;
	event.preventDefault();
	event.stopPropagation();
}

export function blockEscapeDuringDrag(): () => void {
	if (activeDragCount === 0) {
		window.addEventListener("keydown", handleKeyDown, ESCAPE_KEYDOWN_OPTIONS);
	}

	activeDragCount += 1;

	let released = false;

	return () => {
		if (released) return;
		released = true;
		activeDragCount = Math.max(0, activeDragCount - 1);

		if (activeDragCount === 0) {
			window.removeEventListener("keydown", handleKeyDown, ESCAPE_KEYDOWN_OPTIONS);
		}
	};
}
