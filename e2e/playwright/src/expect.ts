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
 * Floor for an attempt's deadline, so an assertion that starts against an
 * already-quiet app still gets one meaningful retry window before the idle
 * budget — possibly spent before the assertion even began — is consulted.
 */
const MIN_ATTEMPT_MS = 1_000;

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

/**
 * Build one assertion attempt bounded at `timeoutMs`. Playwright assertions
 * cannot be canceled, so the deadline is what makes an attempt abandonable —
 * and `configure` sets it without touching the call's arguments, since matchers
 * take their options in varying positions (and some take an object as the
 * expected value).
 */
function attempt(
	subject: unknown,
	modifiers: Modifiers,
	matcher: string,
	args: unknown[],
	timeoutMs: number,
): unknown {
	// The matcher surface is dynamic by nature; this is the one place that has
	// to reach through it untyped.
	let node: any = base.configure({ timeout: timeoutMs })(subject as any);
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
	// Attempts are sized to the remaining idle budget — the same argument the
	// wait watchdog makes: while the app is active, idleness cannot reach the
	// budget sooner than the remainder, so one attempt spans exactly that and
	// the expiry check at its boundary is never late. The assertion polls
	// usefully for the whole span, and a typical one needs one or two attempts
	// instead of a fixed-chunk parade.
	function remaining() {
		return IDLE_BUDGET_MS - idleSince(page, startedAt);
	}
	const first = attempt(subject, modifiers, matcher, args, Math.max(remaining(), MIN_ATTEMPT_MS));
	// A matcher on a plain value resolves or throws synchronously; nothing to retry.
	if (!(first instanceof Promise)) return first;

	return (async () => {
		let pending = first;
		let failure: unknown;
		let remainingMs: number;
		do {
			try {
				return await pending;
			} catch (error) {
				failure = error;
			}
			remainingMs = remaining();
			if (remainingMs > 0) {
				pending = attempt(subject, modifiers, matcher, args, remainingMs) as Promise<unknown>;
			}
		} while (remainingMs > 0);
		throw failure;
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
			// Matchers are string-named; symbol-named functions belong to
			// Node/inspect plumbing and must pass through untouched.
			if (typeof value !== "function" || typeof property !== "string") return value;
			return (...args: unknown[]) => retryWhileAppWorks(subject, page, modifiers, property, args);
		},
	});
}

export const expect = new Proxy(base, {
	get(target, property, receiver) {
		const value = Reflect.get(target, property, receiver);
		// Bind the helpers (`poll`, `configure`, `soft`, …) to the real expect,
		// which otherwise run with the proxy as `this`. Only own string-named
		// properties: inherited function plumbing (`call`, `apply`, `bind`)
		// must stay unbound.
		if (
			typeof value === "function" &&
			typeof property === "string" &&
			Object.prototype.hasOwnProperty.call(target, property)
		) {
			return value.bind(target);
		}
		return value;
	},
	apply(target, thisArg, args: unknown[]) {
		const assertion = Reflect.apply(target, thisArg, args) as object;
		const page = pageOf(args[0]);
		if (!page) return assertion;
		return wrapAssertion(assertion, args[0], page, []);
	},
});
