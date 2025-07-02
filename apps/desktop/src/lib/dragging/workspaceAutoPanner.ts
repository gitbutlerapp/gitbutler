import { on } from 'svelte/events';

// 0.25 indicates that each gutter is 25% of the workspace width.
const GUTTER_WIDTH_RATIO = 0.25;
const PAN_SPEED_MULTIPLIER = 10;

/**
 * Automatically pans the workspace, usually when something is being dragged.
 */
export class WorkspaceAutoPanner {
	private scrollSpeed = 0;

	constructor(private readonly workspace: HTMLElement) {}

	private panningCount = 0;
	private scrollingSubscription?: () => void;
	private eventsSubscription?: () => void;
	/**
	 * We only want to pan when things are being dragged. Something can call
	 * this
	 * whenever panning has started, and then call it's unsubscriber when they
	 * want to stop panning, and if there are no other callers wanting to pan,
	 * we stop panning.
	 */
	enablePanning() {
		this.panningCount++;
		if (this.panningCount === 1) {
			this.scrollingSubscription = this.updateScrollPosition();
			this.eventsSubscription = this.registerListeners();
		}

		return () => {
			this.panningCount--;
			if (this.panningCount === 0) {
				this.scrollingSubscription?.();
				this.eventsSubscription?.();
				this.scrollSpeed = 0;
			}
		};
	}

	/**
	 * Set the scroll speed based on the mouse position.
	 *
	 * As the mouse enters the gutter, the scroll speed will start off as 0, and
	 * increase linearly up to 1 or -1 depending on whether the mouse is going
	 * left or right.
	 *
	 * The scroll speed will be negative in the left gutter, and positive in the
	 * right gutter.
	 */
	private setScrollSpeed(e: MouseEvent) {
		// The position of the mouse relative to the LHS of the workspace
		// element
		const mouseLeft = e.clientX - this.workspace.getBoundingClientRect().left;

		const gutterWidth = this.workspace.clientWidth * GUTTER_WIDTH_RATIO;

		if (mouseLeft >= 0 && mouseLeft <= gutterWidth) {
			// In left gutter
			this.scrollSpeed = (mouseLeft - gutterWidth) / gutterWidth;
		} else if (
			mouseLeft >= this.workspace.clientWidth - gutterWidth &&
			mouseLeft <= this.workspace.clientWidth
		) {
			// In right gutter
			this.scrollSpeed = (mouseLeft - (this.workspace.clientWidth - gutterWidth)) / gutterWidth;
		} else {
			this.scrollSpeed = 0;
		}
	}

	private registerListeners() {
		const mouseMove = on(document, 'mousemove', (e: MouseEvent) => this.setScrollSpeed(e), {
			capture: true
		});
		const onDrag = on(document, 'drag', (e: MouseEvent) => this.setScrollSpeed(e), {
			capture: true
		});

		return () => {
			mouseMove();
			onDrag();
		};
	}

	private get shouldPan() {
		return this.panningCount > 0;
	}

	/**
	 * Starts an interval that updates the scroll position based on the current
	 * scroll speed.
	 */
	private updateScrollPosition() {
		const interval = setInterval(() => {
			if (this.workspace && this.scrollSpeed !== 0 && this.shouldPan) {
				this.workspace.scrollLeft += this.scrollSpeed * PAN_SPEED_MULTIPLIER;
			}
		}, 10);
		return () => clearInterval(interval);
	}
}
