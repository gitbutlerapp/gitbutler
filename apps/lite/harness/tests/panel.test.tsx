import type { WatcherEvent, WorktreeChanges } from "@gitbutler/but-sdk";
import { expect, test, vi } from "vitest";
import createPanel from "../browser/index.tsx";
import { createFakeTransport, createWatcherHandlers, type FakeHandlers } from "./fake-transport.ts";
import {
	fixtureCommit,
	fixtureFileChange,
	fixtureForgeInfo,
	fixtureForgeReview,
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

/**
 * @param seenMarks watermarks to start from, as the app would have stamped on
 * an earlier run. Local storage is shared across the tests in this file, so it
 * is cleared either way.
 */
/** The inbox as the detector wrote it, straight from the store's key. */
const inboxEntries = (): Array<{ kind: string; review: number; author: string | null }> =>
	JSON.parse(localStorage.getItem(`pr_activity_inbox:v1:${PROJECT_ID}`) ?? "[]") as Array<{
		kind: string;
		review: number;
		author: string | null;
	}>;

const mountPanel = (handlers: FakeHandlers, seenMarks?: Record<number, string>) => {
	localStorage.clear();
	if (seenMarks !== undefined)
		localStorage.setItem(`pr_activity_seen:v1:${PROJECT_ID}`, JSON.stringify(seenMarks));
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

test("someone else's review activity files one coalesced inbox entry and the unread dot", async () => {
	// A mutable listing: the first answer is the detector's baseline, the
	// second is the activity, as two forge polls would return.
	let review = fixtureForgeReview({ modifiedAt: "2026-01-01T10:00:00Z" });
	let comments: Array<unknown> = [];

	const panel = mountPanel({
		headInfo: () =>
			fixtureHeadInfo([[fixtureSegment({ branch: review.sourceBranch, commits: [] })]]),
		changesInWorktree: () => fixtureWorktreeChanges([]),
		forgeInfo: () => fixtureForgeInfo(),
		listReviews: () => [review],
		currentForgeLogin: () => "me",
		listReviewComments: () => comments,
		listReviewSubmissions: () => [],
		listReviewTimelineEvents: () => [],
	});

	// The PR chip proves the baseline listing landed, and that nothing is
	// unread yet — history must never replay as notifications.
	await vi.waitFor(() => expect(panel.container.textContent).toContain("PR"), settle);
	expect(document.querySelector('[title="New activity on this pull request"]')).toBeNull();

	// Someone comments; the forge bumps the review and a fetch notices.
	review = { ...review, modifiedAt: "2026-01-01T11:00:00Z" };
	comments = [
		{
			id: 1,
			body: "Have you considered a smaller diff?",
			author: { id: 2, login: "alice", name: null, email: null, avatarUrl: null, isBot: false },
			createdAt: "2026-01-01T10:30:00Z",
			modifiedAt: null,
			htmlUrl: "",
			reactions: [],
		},
	];
	const eventChannel = panel.watcher.channels.at(0);
	if (eventChannel === undefined) throw new Error("no watcher subscription armed");
	const event: WatcherEvent = { name: "gitFetch", payload: { type: "gitFetch", subject: null } };
	panel.push(eventChannel, event);

	// One coalesced, attributed inbox entry — and the unread dot alongside it.
	await vi.waitFor(() => expect(inboxEntries()).toHaveLength(1), settle);
	expect(inboxEntries()[0]).toMatchObject({ kind: "comment", review: 7, author: "alice" });
	await vi.waitFor(
		() =>
			expect(document.querySelector('[title="New activity on this pull request"]')).not.toBeNull(),
		settle,
	);

	panel.unmount();
});

test("a mention toasts even when the review's branch is not in the workspace", async () => {
	// One applied review to anchor the baseline, and one on a branch the
	// workspace does not hold — only a mention may speak for the latter.
	const mine = fixtureForgeReview({ modifiedAt: "2026-01-01T10:00:00Z" });
	let outside = fixtureForgeReview({
		number: 8,
		sourceBranch: "a-colleagues-branch",
		modifiedAt: "2026-01-01T10:00:00Z",
	});
	let comments: Array<unknown> = [];

	const panel = mountPanel({
		headInfo: () => fixtureHeadInfo([[fixtureSegment({ branch: mine.sourceBranch, commits: [] })]]),
		changesInWorktree: () => fixtureWorktreeChanges([]),
		forgeInfo: () => fixtureForgeInfo(),
		listReviews: () => [mine, outside],
		currentForgeLogin: () => "me",
		listReviewComments: (params: { reviewId: number }) =>
			params.reviewId === outside.number ? comments : [],
		listReviewSubmissions: () => [],
		listReviewTimelineEvents: () => [],
	});

	await vi.waitFor(() => expect(panel.container.textContent).toContain("PR"), settle);

	outside = { ...outside, modifiedAt: "2026-01-01T11:00:00Z" };
	comments = [
		{
			id: 1,
			body: "wdyt @me?",
			author: { id: 2, login: "alice", name: null, email: null, avatarUrl: null, isBot: false },
			createdAt: "2026-01-01T10:30:00Z",
			modifiedAt: null,
			htmlUrl: "",
			reactions: [],
		},
	];
	const eventChannel = panel.watcher.channels.at(0);
	if (eventChannel === undefined) throw new Error("no watcher subscription armed");
	panel.push(eventChannel, {
		name: "gitFetch",
		payload: { type: "gitFetch", subject: null },
	} satisfies WatcherEvent);

	await vi.waitFor(
		() =>
			expect(inboxEntries().find((entry) => entry.review === 8)).toMatchObject({ kind: "mention" }),
		settle,
	);

	panel.unmount();
});
