/**
 * ScrollSelectionLock
 *
 * Prevents VirtualList's scroll-driven `onVisibleChange` events from overwriting a
 * programmatically clicked selection before — and while — the target item is visible.
 *
 * Two-phase lifecycle:
 *   Phase 1 — item not yet visible: hold the locked index, ignore range.start.
 *   Phase 2 — item is visible:      keep it pinned while it stays in the viewport.
 *   Release  — item leaves view:    clear the lock; caller resumes auto-follow.
 *
 * Usage:
 *   const lock = new ScrollSelectionLock(initialIndex);
 *
 *   // on user click / programmatic jump:
 *   lock.set(index);
 *
 *   // inside onVisibleChange:
 *   const active = lock.resolve(range);   // use `active` as the authoritative index
 */
export class ScrollSelectionLock {
	#index = $state<number | undefined>();
	#hasBeenVisible = $state(false);

	constructor(initialIndex?: number) {
		this.#index = initialIndex;
	}

	/** Arm the lock for a new target index (call on click / jumpToIndex). */
	set(index: number) {
		this.#index = index;
		this.#hasBeenVisible = false;
	}

	/**
	 * Resolve the authoritative active index for this scroll event.
	 * Pass the raw `range` from VirtualList's `onVisibleChange`.
	 * Returns the index that should be treated as active.
	 */
	resolve(range: { start: number; end: number }): number {
		if (this.#index === undefined) {
			return range.start;
		}

		const lockVisible = this.#index >= range.start && this.#index < range.end;

		if (!this.#hasBeenVisible) {
			// Phase 1: item not yet in view — hold the lock.
			if (lockVisible) {
				this.#hasBeenVisible = true;
			}
			return this.#index;
		}

		if (lockVisible) {
			// Phase 2: item still visible — stay pinned.
			return this.#index;
		}

		// Release: item has left the viewport — resume auto-follow.
		this.#index = undefined;
		this.#hasBeenVisible = false;
		return range.start;
	}
}
