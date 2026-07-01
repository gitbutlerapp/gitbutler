import { on } from "svelte/events";

/**
 * Using the mouse to scroll the element.
 */
export class HorizontalPanner {
	private isPanning = false;
	private panStartX = 0;
	private panStartScrollLeft = 0;
	private originalCursor: string = "";

	constructor(private readonly element: HTMLElement) {}

	private handleMouseMove(e: MouseEvent) {
		e.preventDefault();

		if (!this.isPanning) return;

		const deltaX = e.clientX - this.panStartX;
		this.element.scrollLeft = this.panStartScrollLeft - deltaX;
	}

	registerListeners() {
		const mouseDown = on(this.element, "mousedown", this.handleMouseDown.bind(this), {});
		const mouseUp = on(document, "mouseup", this.stopPanning.bind(this), {});

		return () => {
			mouseDown();
			mouseUp();
			this.stopPanning();
		};
	}

	private mouseMoveSubscription?: () => void;

	private clearSubscription() {
		this.mouseMoveSubscription?.();
	}

	private stopPanning() {
		if (this.isPanning) {
			this.isPanning = false;
			this.element.style.cursor = this.originalCursor;
		}
		this.clearSubscription();
	}

	private handleMouseDown(e: MouseEvent) {
		if (e.button !== 0) return;
		if (!(e.target instanceof HTMLElement)) return;

		// Exclude clicks on interactive elements
		if (e.target.closest("button, a, input, select, textarea")) return;
		if (e.target.closest("[data-remove-from-panning]")) return;

		this.isPanning = true;
		this.panStartX = e.clientX;
		this.panStartScrollLeft = this.element.scrollLeft;

		// Store original cursor and set to grabbing
		this.originalCursor = this.element.style.cursor;
		this.element.style.cursor = "grabbing";

		e.preventDefault();

		// Make sure we clear any old subscriptions.
		this.clearSubscription();
		this.mouseMoveSubscription = on(document, "mousemove", this.handleMouseMove.bind(this));
	}
}
