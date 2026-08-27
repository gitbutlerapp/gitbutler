/**
 * jsdom is structure-only; stub the layout-adjacent APIs the components touch.
 * Anything needing real layout (CodeView) belongs on the CDP rig, not here.
 */

class ResizeObserverStub {
	private readonly callback: ResizeObserverCallback;

	constructor(callback: ResizeObserverCallback) {
		this.callback = callback;
	}

	observe(target: Element): void {
		queueMicrotask(() => {
			if (!target.isConnected) return;
			const blockSize = target.hasAttribute("data-index") ? 28 : 800;
			this.callback(
				[
					{
						target,
						borderBoxSize: [{ inlineSize: 800, blockSize }],
					} as unknown as ResizeObserverEntry,
				],
				this,
			);
		});
	}
	unobserve(): void {}
	disconnect(): void {}
	takeRecords(): Array<ResizeObserverEntry> {
		return [];
	}
}

class WorkerStub {
	postMessage(): void {}
	terminate(): void {}
	addEventListener(): void {}
	removeEventListener(): void {}
}

const globals = globalThis as unknown as Record<string, unknown>;

globals.ResizeObserver ??= ResizeObserverStub;
globals.Worker ??= WorkerStub;
// Node-environment tests (the watcher host) share this setup but have no DOM.
if (typeof Element !== "undefined")
	// jsdom's Element lacks scrollIntoView at runtime despite the DOM type.
	Element.prototype.scrollIntoView = () => {};

// hotkeys.ts reads `window.lite.platform` at module scope — before any test
// can run `createPanel`, which installs the real api over this placeholder.
// (The IIFE build has no such gap: the read is a compile-time define there.)
globals.lite ??= { platform: "darwin" };
