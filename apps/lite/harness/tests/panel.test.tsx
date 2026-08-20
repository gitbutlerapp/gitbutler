import type { WatcherEvent, WorktreeChanges } from "@gitbutler/but-sdk";
import { expect, test, vi } from "vitest";
import createPanel from "../browser/index.tsx";
import { createFakeTransport, createWatcherHandlers, type FakeHandlers } from "./fake-transport.ts";
import {
	fixtureCommit,
	fixtureFileChange,
	fixtureHeadInfo,
	fixtureSegment,
	fixtureWorktreeChanges,
	globalHandlers,
} from "./fixtures.ts";

/**
 * The jsdom half of the verification rig: mount the real panel bundle over a
 * fake transport and assert on structure. Layout (CodeView) is out of scope
 * here — that is the CDP rig's job.
 */

const PROJECT_ID = "fixture-project";

/**
 * `vi.waitFor` allows one second by default, which a warm machine clears in
 * about half that — and a cold CI runner does not. Mounting the panel builds
 * a React root, a store and a query client before the first paint, so the
 * wait is for real work, not a race we could tighten.
 */
const settle = { timeout: 15_000 } as const;

const mountPanel = (handlers: FakeHandlers) => {
	const watcher = createWatcherHandlers();
	const fake = createFakeTransport({
		...globalHandlers(PROJECT_ID),
		...watcher.handlers,
		...handlers,
	});
	const app = createPanel({ transport: fake.transport, projectId: PROJECT_ID });
	const container = document.createElement("div");
	document.body.append(container);
	app.mount(container);

	return {
		container,
		watcher,
		...fake,
		unmount: () => {
			app.unmount();
			container.remove();
		},
	};
};

test("renders the applied branches, their commits, and the uncommitted files", async () => {
	const headInfo = fixtureHeadInfo([
		[
			fixtureSegment({
				branch: "feature-one",
				commits: [fixtureCommit({ id: "a".repeat(40), message: "Add the first feature" })],
			}),
		],
		[fixtureSegment({ branch: "feature-two", commits: [] })],
	]);
	const worktreeChanges = fixtureWorktreeChanges([fixtureFileChange("src/edited-file.ts")]);

	const panel = mountPanel({
		headInfo: () => headInfo,
		changesInWorktree: () => worktreeChanges,
	});

	await vi.waitFor(() => {
		expect(panel.container.textContent).toContain("feature-one");
		expect(panel.container.textContent).toContain("Add the first feature");
		expect(panel.container.textContent).toContain("feature-two");
		expect(panel.container.textContent).toContain("edited-file.ts");
	}, settle);

	panel.unmount();
});

test("a failed mutation surfaces the declared toast", async () => {
	const headInfo = fixtureHeadInfo([
		[
			fixtureSegment({
				branch: "feature-one",
				commits: [fixtureCommit({ id: "b".repeat(40), message: "A commit to push" })],
			}),
		],
	]);
	const panel = mountPanel({
		headInfo: () => headInfo,
		changesInWorktree: () => fixtureWorktreeChanges([]),
	});

	await vi.waitFor(() => expect(panel.container.textContent).toContain("feature-one"), settle);
	const push = [...panel.container.querySelectorAll("button")].find(
		(button) => button.textContent === "Push",
	);
	if (!push) throw new Error("no Push button rendered");
	push.click();

	// The push endpoint has no fake handler, so the mutation rejects and the
	// declared failure toast must appear. Toasts portal to the body, so
	// assert there rather than inside the container.
	await vi.waitFor(() => expect(document.body.textContent).toContain("Failed to push"), settle);

	panel.unmount();
});

test("a watcher event refreshes the uncommitted files", async () => {
	// A mutable source: the event announces the change, and the handler gives
	// the same answer to any consumer that refetches instead.
	let worktree: WorktreeChanges = fixtureWorktreeChanges([]);
	const panel = mountPanel({
		headInfo: () => fixtureHeadInfo([]),
		changesInWorktree: () => worktree,
	});

	await vi.waitFor(
		() => expect(panel.container.textContent).toContain("Nothing to commit"),
		settle,
	);

	// The mount armed exactly one subscription with the host.
	expect(panel.watcher.channels).toHaveLength(1);
	const eventChannel = panel.watcher.channels.at(0);
	if (eventChannel === undefined) throw new Error("unreachable: just asserted");
	expect(panel.subscribedChannels()).toContain(eventChannel);

	worktree = fixtureWorktreeChanges([fixtureFileChange("src/new-file.ts")]);
	const event: WatcherEvent = {
		name: "worktreeChanges",
		payload: { type: "worktreeChanges", subject: { changes: worktree } },
	};
	panel.push(eventChannel, event);

	await vi.waitFor(() => expect(panel.container.textContent).toContain("new-file.ts"), settle);

	panel.unmount();
});
