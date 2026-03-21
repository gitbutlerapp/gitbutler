/**
 * Tracks the number of currently in-flight backend operations.
 * Uses a plain counter (not $state) to avoid interfering with
 * Svelte's reactive effect scheduling when called from tauriInvoke.
 */

let activeCount = 0;
let flushScheduled = false;
let listeners: Array<(count: number) => void> = [];

/**
 * Schedule a deferred flush so $state mutations never happen
 * synchronously inside the tauriInvoke call stack.
 */
function scheduleFlush() {
	if (!flushScheduled) {
		flushScheduled = true;
		queueMicrotask(() => {
			flushScheduled = false;
			for (const listener of listeners) {
				listener(activeCount);
			}
		});
	}
}

export function trackOperation<T>(promise: Promise<T>): Promise<T> {
	activeCount++;
	scheduleFlush();
	return promise.finally(() => {
		activeCount--;
		scheduleFlush();
	});
}

/**
 * Reactive indicator that subscribes to operation count changes.
 * Uses a minimum visible duration so fast operations are still noticeable.
 */
export class ActiveOperationIndicator {
	visible = $state(false);
	private hideTimeout: ReturnType<typeof setTimeout> | undefined;
	private readonly minVisibleMs: number;
	private visibleSince: number | undefined;
	private unsubscribe: (() => void) | undefined;

	constructor(minVisibleMs = 150) {
		this.minVisibleMs = minVisibleMs;
		const onCountChange = (count: number) => {
			if (count > 0) {
				clearTimeout(this.hideTimeout);
				if (!this.visible) {
					this.visibleSince = performance.now();
				}
				this.visible = true;
			} else if (this.visible) {
				const elapsed = performance.now() - (this.visibleSince ?? 0);
				const remaining = Math.max(0, this.minVisibleMs - elapsed);
				if (remaining === 0) {
					this.visible = false;
					this.visibleSince = undefined;
				} else {
					this.hideTimeout = setTimeout(() => {
						this.visible = false;
						this.visibleSince = undefined;
					}, remaining);
				}
			}
		};
		listeners.push(onCountChange);
		this.unsubscribe = () => {
			listeners = listeners.filter((l) => l !== onCountChange);
		};
		if (activeCount > 0) {
			this.visibleSince = performance.now();
			this.visible = true;
		}
	}

	destroy() {
		this.unsubscribe?.();
		clearTimeout(this.hideTimeout);
	}
}
