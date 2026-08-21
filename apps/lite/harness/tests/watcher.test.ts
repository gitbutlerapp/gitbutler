// @vitest-environment node
import { beforeEach, expect, test, vi } from "vitest";

/** Hold every started watcher so the test can see what was stopped. */
const handles: Array<{ stopped: boolean }> = [];
let release: (() => void) | undefined;

vi.mock("@gitbutler/but-sdk", () => ({
	watcherStart: async () => {
		// Park the start until the test lets it finish, so `stopAll` can land
		// in the middle exactly as it does when a panel closes.
		await new Promise<void>((resolve) => {
			release = resolve;
		});
		const handle = {
			stopped: false,
			stop() {
				this.stopped = true;
			},
		};
		handles.push(handle);
		return handle;
	},
}));

const { createHostWatcher } = await import("../node/watcher.ts");

// Both are asserted on, so neither may carry over from a previous test.
beforeEach(() => {
	handles.length = 0;
	release = undefined;
});

test("a watcher that finishes starting after shutdown is stopped, not registered", async () => {
	const watcher = createHostWatcher(() => {});

	const subscribing = watcher.subscribe("project");
	await vi.waitFor(() => expect(release).toBeDefined());

	watcher.stopAll();
	release?.();

	await expect(subscribing).rejects.toThrow(/shutting down/);
	expect(handles).toHaveLength(1);
	expect(handles[0]?.stopped, "the late handle is stopped rather than left running").toBe(true);
	expect(watcher.stopAll(), "no subscription survived shutdown").toBe(0);
});

test("a subscribe arriving after shutdown never starts a watcher", async () => {
	const watcher = createHostWatcher(() => {});

	watcher.stopAll();

	await expect(watcher.subscribe("project")).rejects.toThrow(/shutting down/);
	expect(release, "watcherStart was never called").toBeUndefined();
	expect(handles, "no SDK watcher was started").toHaveLength(0);
});
