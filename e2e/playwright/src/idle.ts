import type { Page, Request } from "@playwright/test";

/**
 * How long the app may sit completely idle — nothing in flight — while a wait
 * or assertion is still unsatisfied, before we call it broken.
 *
 * A wall-clock deadline can only ever be a guess at how slow a runner is, and
 * guessing low is what turns "this machine was busy" into a failed test. This
 * budget measures something the machine's speed cannot inflate instead: how
 * long the app has done *nothing at all*. A slow round trip keeps a request in
 * flight, so the clock does not run and the wait tolerates it for as long as it
 * takes. State that is never coming leaves the app quiet, and that is reported
 * in seconds rather than at the test's deadline.
 */
export const IDLE_BUDGET_MS = 10_000;

type Activity = {
	pending: Set<Request>;
	lastActiveAt: number;
};

/**
 * Requests in flight per page, tracked once and shared by every wait and
 * assertion.
 *
 * A set rather than a counter so a duplicated terminal event cannot drift it
 * below zero. If a request somehow never settles, the set stays non-empty and
 * the budget simply never expires — waiting then falls back to the per-test
 * timeout, which is the safe direction: a lost event can delay a failure, never
 * manufacture one.
 */
const activityByPage = new WeakMap<Page, Activity>();

function activityOf(page: Page): Activity {
	const existing = activityByPage.get(page);
	if (existing) return existing;

	const activity: Activity = { pending: new Set(), lastActiveAt: Date.now() };
	activityByPage.set(page, activity);
	function started(request: Request) {
		activity.pending.add(request);
		activity.lastActiveAt = Date.now();
	}
	function settled(request: Request) {
		activity.pending.delete(request);
		activity.lastActiveAt = Date.now();
	}
	page.on("request", started);
	page.on("requestfinished", settled);
	page.on("requestfailed", settled);
	return activity;
}

/**
 * How long the app has been idle, counting only from `since`.
 *
 * Idleness accumulated before we started waiting says nothing about the wait:
 * a test spends whole seconds in `runScript` shelling out to git, and the app
 * is legitimately quiet throughout. Measuring from `since` means the budget
 * always answers "has the app done anything since we started waiting for this".
 */
export function idleSince(page: Page, since: number): number {
	const activity = activityOf(page);
	if (activity.pending.size > 0) return 0;
	return Date.now() - Math.max(activity.lastActiveAt, since);
}

/** Start tracking now, so activity is not missed between here and the first check. */
export function watchIdle(page: Page): void {
	activityOf(page);
}
