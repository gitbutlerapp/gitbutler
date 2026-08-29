import { IDLE_BUDGET_MS, idleSince, watchIdle } from "./idle.ts";
import { expect as base, type Locator, type Page } from "@playwright/test";

/**
 * `expect` that tolerates a slow runner but not a silent app.
 *
 * Playwright's assertions already retry until they pass; the only question is
 * how long they may take, and `playwright.config.ts` deliberately gives them no
 * deadline of its own. That makes them tolerate any slowness, but it also means
 * a genuinely wrong assertion runs until the per-test timeout.
 *
 * So each assertion is retried in bounded attempts, and gives up only once the
 * app has been idle for {@link IDLE_BUDGET_MS} — the same rule the wait helpers
 * in `util.ts` use. While the app is working the assertion keeps retrying, so a
 * slow round trip costs nothing; once it goes quiet, the failure surfaces in
 * seconds, carrying Playwright's own expected/received message because the
 * error thrown is the last attempt's.
 *
 * Assertions on plain values are untouched: they cannot wait on anything, and
 * there is no page whose activity would say when to give up.
 */

/**
 * Attempts are bounded so the loop regains control to ask whether the app is
 * still working. Short enough that a failure is reported promptly after the app
 * goes quiet, long enough that a passing assertion rarely needs a second one.
 */
const ATTEMPT_MS = 1_000;

/**
 * `configure` sets the attempt deadline without touching the call's arguments —
 * matchers take their options in varying positions (and some take an object as
 * the expected value), so injecting a `timeout` into the arguments would have
 * to guess which is which.
 */
const bounded = base.configure({ timeout: ATTEMPT_MS });

/** Chain of modifiers (`not`, `resolves`, `rejects`) applied before the matcher. */
type Modifiers = readonly string[];

function pageOf(subject: unknown): Page | undefined {
	if (!subject || typeof subject !== "object") return undefined;
	const candidate = subject as Partial<Locator> & Partial<Page>;
	if (typeof candidate.elementHandle === "function" && typeof candidate.page === "function") {
		return (subject as Locator).page();
	}
	if (typeof candidate.goto === "function" && typeof candidate.context === "function") {
		return subject as Page;
	}
	return undefined;
}

/** Build a fresh bounded assertion, so each attempt starts its own retry window. */
function attempt(
	subject: unknown,
	modifiers: Modifiers,
	matcher: string,
	args: unknown[],
): unknown {
	// The matcher surface is dynamic by nature; this is the one place that has
	// to reach through it untyped.

	let node: any = bounded(subject as any);
	for (const modifier of modifiers) node = node[modifier];
	return node[matcher](...args);
}

function retryWhileAppWorks(
	subject: unknown,
	page: Page,
	modifiers: Modifiers,
	matcher: string,
	args: unknown[],
): unknown {
	watchIdle(page);
	const startedAt = Date.now();
	const first = attempt(subject, modifiers, matcher, args);
	// A matcher on a plain value resolves or throws synchronously; nothing to retry.
	if (!(first instanceof Promise)) return first;

	return (async () => {
		let pending = first;
		for (;;) {
			try {
				return await pending;
			} catch (error) {
				if (idleSince(page, startedAt) >= IDLE_BUDGET_MS) throw error;
				pending = attempt(subject, modifiers, matcher, args) as Promise<unknown>;
			}
		}
	})();
}

function wrapAssertion(
	assertion: object,
	subject: unknown,
	page: Page,
	modifiers: Modifiers,
): object {
	return new Proxy(assertion, {
		get(target, property, receiver) {
			const value = Reflect.get(target, property, receiver);
			if (property === "not" || property === "resolves" || property === "rejects") {
				return wrapAssertion(value as object, subject, page, [...modifiers, property]);
			}
			if (typeof value !== "function") return value;
			return (...args: unknown[]) =>
				retryWhileAppWorks(subject, page, modifiers, property as string, args);
		},
	});
}

export const expect = new Proxy(base, {
	apply(target, thisArg, args: unknown[]) {
		const assertion = Reflect.apply(target, thisArg, args) as object;
		const page = pageOf(args[0]);
		if (!page) return assertion;
		return wrapAssertion(assertion, args[0], page, []);
	},
});
