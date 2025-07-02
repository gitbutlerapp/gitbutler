import { on } from 'svelte/events';

/**
 * Using the mouse to scroll the element.
 */
export class HorizontalPanner {
	private isPanning = false;
	private panStartX = 0;
	private panStartScrollLeft = 0;

	constructor(private readonly element: HTMLElement) {}

	private handleMouseMove(e: MouseEvent) {
		if (!this.isPanning) return;

		const deltaX = e.clientX - this.panStartX;
		this.element.scrollLeft = this.panStartScrollLeft - deltaX;
	}

	registerListeners() {
		const mouseDown = on(this.element, 'mousedown', this.handleMouseDown.bind(this), {
			capture: true
		});
		const mouseUp = on(this.element, 'mouseup', this.clearSubscription.bind(this), {
			capture: true
		});
		const mouseLeave = on(this.element, 'mouseleave', this.clearSubscription.bind(this), {
			capture: true
		});

		return () => {
			mouseDown();
			mouseUp();
			mouseLeave();
			this.clearSubscription();
		};
	}

	private mouseMoveSubscription?: () => void;
	private clearSubscription() {
		this.mouseMoveSubscription?.();
	}

	private handleMouseDown(e: MouseEvent) {
		if (e.button !== 0) return;

		if (!(e.target instanceof HTMLElement)) return;

		// Exclude clicks on interactive elements
		if (e.target.closest('button, a, input, select, textarea')) return;
		if (e.target.closest('[data-remove-from-panning]')) return;
		this.isPanning = true;
		this.panStartX = e.clientX;
		this.panStartScrollLeft = this.element.scrollLeft;

		// Make sure we clear any old subscriptions.
		this.clearSubscription();
		this.mouseMoveSubscription = on(this.element, 'mousemove', this.handleMouseMove.bind(this));
	}
}
