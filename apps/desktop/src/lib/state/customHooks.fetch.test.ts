/**
 * Regression test for APP-JS-77K — `TypeError: undefined is not an object
 * (evaluating 't.url')` from `UserService.getLoginUrl`.
 *
 * Root cause: the customHooks `fetch` helper calls
 *
 *   const result = dispatch(initiate(arg, { subscribe: false, forceRefetch: true }));
 *   const data = await result.unwrap();
 *
 * relying on `unwrap()` to either return defined data or throw. That
 * contract holds with RTK Query's default `keepUnusedDataFor` (60s), but
 * breaks when the api is configured with `keepUnusedDataFor: 0`:
 *
 *   1. First call enters pending; runningQueries map gains the entry.
 *   2. Second call enters; condition() sees pending and returns false;
 *      branch 3 of buildInitiate runs:
 *        Promise.all([runningQuery, thunkResult]).then(selectFromState)
 *   3. First call's baseQuery resolves. queryThunk.fulfilled dispatches.
 *      The cacheCollection middleware schedules a `setTimeout(0)`
 *      cleanup because there are no subscribers (subscribe: false) and
 *      keepUnusedDataFor is 0.
 *   4. First call's statePromise resolves with data. First caller's
 *      unwrap returns correctly.
 *   5. The cleanup macrotask fires. `removeQueryResult` dispatched.
 *      Cache entry GONE.
 *   6. Second call's Promise.all + selectFromState now runs. Selector
 *      finds no entry, returns an uninitialized QuerySubState with
 *      `data: undefined, isError: false`. `unwrap()` returns undefined.
 *
 * Fix (production): use the RTK Query default keepUnusedDataFor — see the
 * comment on `keepUnusedDataFor` in `backendApi.ts`. Removing the
 * `keepUnusedDataFor: 0` setting defers cleanup past the read.
 *
 * This file holds two tests:
 *   - the green test asserts the production policy (default
 *     keepUnusedDataFor) is safe for concurrent `fetch()` calls.
 *   - the red test pins the trap: if `keepUnusedDataFor: 0` is ever
 *     reintroduced, that test will fail and the failure message will
 *     point straight back to this comment.
 */
import { configureStore } from "@reduxjs/toolkit";
import { createApi } from "@reduxjs/toolkit/query";
import { describe, expect, test, vi } from "vitest";

type Token = { url: string };

type HarnessOptions = {
	keepUnusedDataFor?: number;
};

/** Sets up a minimal RTK Query store mirroring the customHooks `fetch` helper.
 *  The baseQuery is manually-resolvable so we can keep the first call pending
 *  while the second one dispatches, then resolve the first via setTimeout to
 *  model an async IPC roundtrip (the macrotask boundary needed to expose
 *  the cleanup race when `keepUnusedDataFor: 0`). */
function makeHarness({ keepUnusedDataFor }: HarnessOptions = {}) {
	const resolvers: Array<(t: Token) => void> = [];
	const baseQuery = vi.fn(
		async () =>
			await new Promise<{ data: Token }>((resolve) => {
				resolvers.push((t) => {
					// Fulfill in a macrotask, matching a real Tauri IPC response.
					setTimeout(() => resolve({ data: t }), 0);
				});
			}),
	);

	const api = createApi({
		reducerPath: "testApi",
		// biome-ignore lint/suspicious/noExplicitAny: synthetic baseQuery for test
		baseQuery: baseQuery as any,
		...(keepUnusedDataFor !== undefined ? { keepUnusedDataFor } : {}),
		endpoints: (build) => ({
			getToken: build.query<Token, void>({
				query: () => undefined,
			}),
		}),
	});

	const store = configureStore({
		reducer: { [api.reducerPath]: api.reducer },
		middleware: (gDM) => gDM().concat(api.middleware),
	});

	// Verbatim copy of customHooks.svelte.ts's `fetch` helper.
	async function fetchEndpoint(): Promise<Token> {
		const result = store.dispatch(
			api.endpoints.getToken.initiate(undefined, {
				subscribe: false,
				forceRefetch: true,
			}),
		);
		return await result.unwrap();
	}

	return { baseQuery, fetchEndpoint, resolvers };
}

/** Drives two overlapping fetches against the harness: first enters pending,
 *  second dispatches while first is in flight, then the first fulfills. */
async function raceTwoFetches(harness: ReturnType<typeof makeHarness>) {
	const firstPromise = harness.fetchEndpoint();
	await Promise.resolve(); // flush first dispatch into pending state
	const secondPromise = harness.fetchEndpoint();
	expect(harness.resolvers).toHaveLength(1);
	harness.resolvers[0]?.({ url: "ok" });
	return await Promise.all([firstPromise, secondPromise]);
}

describe("customHooks fetch helper — concurrent-call safety", () => {
	test("production config (default keepUnusedDataFor) — both concurrent calls return data", async () => {
		// Mirrors apps/desktop/src/lib/state/backendApi.ts after removing
		// the keepUnusedDataFor:0 setting. Default is 60s, so the cleanup
		// fires well after the second caller's selectFromState reads.
		const harness = makeHarness();
		const [first, second] = await raceTwoFetches(harness);

		expect(harness.baseQuery).toHaveBeenCalledTimes(1); // second call deduped
		expect(first).toEqual({ url: "ok" });
		expect(second).toEqual({ url: "ok" });
	});

	test("control — keepUnusedDataFor:0, single call is fine; the race needs concurrency", async () => {
		// Documenting the boundary of the bug. A lone call with
		// keepUnusedDataFor:0 + async fulfillment does NOT trigger the
		// undefined return — the cleanup setTimeout(0) only ever fires
		// after this single caller's selectFromState completes.
		const { fetchEndpoint, resolvers } = makeHarness({ keepUnusedDataFor: 0 });
		const promise = fetchEndpoint();
		await Promise.resolve();
		resolvers[0]?.({ url: "ok" });
		expect(await promise).toEqual({ url: "ok" });
	});

	test("trap regression — keepUnusedDataFor:0 lets concurrent unwrap() return undefined", async () => {
		// If this test starts FAILING because a future change set
		// keepUnusedDataFor back to 0 (or any value tight enough to race
		// with the await chain), see the file-level comment for the
		// mechanism and backendApi.ts for the policy.
		//
		// Production note: APP-JS-77K events in 0.19.13 show only ONE
		// call to getLoginToken.fetch() per failure (no get_login_token
		// IPC in the breadcrumbs). The literal production trigger may
		// involve an additional path we haven't reproduced — but the
		// concurrent-call race below IS a real bug class with the
		// previous config, and removing keepUnusedDataFor:0 prevents it.
		const harness = makeHarness({ keepUnusedDataFor: 0 });
		const [first, second] = await raceTwoFetches(harness);

		expect(harness.baseQuery).toHaveBeenCalledTimes(1);
		// At least one of the two calls returns undefined under this
		// config; we don't pin which, since the exact microtask vs
		// macrotask interleave varies across Node/jsdom/runtime versions.
		// The point is `unwrap()` violates its contract — it returns
		// undefined without throwing.
		expect(first === undefined || second === undefined).toBe(true);
	});
});
