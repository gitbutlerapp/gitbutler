import { butlerModule } from "$lib/state/butlerModule";
import { ReduxTag } from "$lib/state/tags";
import { configureStore } from "@reduxjs/toolkit";
import { buildCreateApi, coreModule } from "@reduxjs/toolkit/query";
import { flushSync } from "svelte";
import { describe, expect, test, vi } from "vitest";
import type { TauriBaseQueryFn } from "$lib/state/backendQuery";
import type { HookContext } from "$lib/state/context";
import type { PostHogWrapper } from "$lib/telemetry/posthog";

function setup() {
	const capture = vi.fn();
	const baseQueryCalls = vi.fn();
	// Mirrors the store into a signal, as the app's client state does, so a
	// query's derived result follows the store.
	let state = $state.raw<unknown>(undefined);
	const ctx: HookContext = {
		getState: () => state as ReturnType<typeof store.getState>,
		getDispatch: () => store.dispatch,
		posthog: { capture } as unknown as PostHogWrapper,
	};
	async function baseQuery(
		args: Parameters<TauriBaseQueryFn>[0],
		_api: Parameters<TauriBaseQueryFn>[1],
		_extraOptions: Parameters<TauriBaseQueryFn>[2],
	): Promise<Awaited<ReturnType<TauriBaseQueryFn>>> {
		baseQueryCalls();
		if ((args as { fail?: boolean } | undefined)?.fail) {
			return { error: { origin: "ipc", name: "API error", message: "it broke" } };
		}
		return { data: undefined };
	}
	const api = buildCreateApi(
		coreModule(),
		butlerModule(ctx),
	)({
		reducerPath: "backend",
		tagTypes: Object.values(ReduxTag),
		baseQuery,
		endpoints: (build) => ({
			unnamedMutation: build.mutation<void, { fail?: boolean }>({
				extraOptions: { command: "some_command" },
				query: (args) => args,
			}),
			namedMutation: build.mutation<void, { fail?: boolean }>({
				extraOptions: { command: "some_command", actionName: "Some Action" },
				query: (args) => args,
			}),
			query: build.query<void, { fail?: boolean }>({
				extraOptions: { command: "some_query" },
				query: (args) => args,
			}),
		}),
	});
	const store = configureStore({
		reducer: { [api.reducerPath]: api.reducer },
		middleware: (getDefaultMiddleware) => getDefaultMiddleware().concat(api.middleware),
	});
	state = store.getState();
	store.subscribe(() => {
		state = store.getState();
	});
	return { api, capture, baseQueryCalls };
}

function capturedEventNames(capture: ReturnType<typeof vi.fn>) {
	return capture.mock.calls.map((call) => call[0]);
}

describe("mutation tracking", () => {
	test("a failed unnamed mutation emits tauri_command but no legacy event", async () => {
		const { api, capture } = setup();

		await expect(api.endpoints.unnamedMutation.mutate({ fail: true })).rejects.toMatchObject({
			message: "it broke",
		});

		expect(capturedEventNames(capture)).toEqual(["tauri_command"]);
		expect(capture).toHaveBeenCalledWith(
			"tauri_command",
			expect.objectContaining({ command: "some_command", failure: true }),
		);
	});

	test("a successful unnamed mutation emits tauri_command but no legacy event", async () => {
		const { api, capture } = setup();

		await api.endpoints.unnamedMutation.mutate({});

		expect(capturedEventNames(capture)).toEqual(["tauri_command"]);
	});

	test("a named mutation still emits the legacy events", async () => {
		const { api, capture } = setup();

		await api.endpoints.namedMutation.mutate({});
		await expect(api.endpoints.namedMutation.mutate({ fail: true })).rejects.toMatchObject({
			message: "it broke",
		});

		expect(capturedEventNames(capture)).toEqual([
			"tauri_command",
			"Some Action Successful",
			"tauri_command",
			"Some Action Failed",
		]);
	});
});

describe("useQuery subscriptions", () => {
	test("changing the subscription options of a failed query does not re-run it", async () => {
		const { api, baseQueryCalls } = setup();
		const args = { fail: true };

		let query: ReturnType<typeof api.endpoints.query.useQuery> | undefined;
		let subscribed = false;
		const dispose = $effect.root(() => {
			query = api.endpoints.query.useQuery(args, {
				subscriptionOptions: { pollingInterval: 60_000 },
			});
			// Reading the result inside an effect is what subscribes.
			$effect(() => {
				subscribed = query?.result !== undefined;
			});
		});
		flushSync();
		expect(subscribed).toBe(true);
		await vi.waitFor(() => expect(query?.result.isError).toBe(true));
		expect(baseQueryCalls).toHaveBeenCalledTimes(1);

		// Widening the interval in place leaves the failed request alone...
		query?.updateSubscriptionOptions({ pollingInterval: 300_000 });
		flushSync();
		await new Promise((resolve) => setTimeout(resolve, 50));
		expect(baseQueryCalls).toHaveBeenCalledTimes(1);

		// ...while a fresh subscription re-runs a never-fulfilled query, which
		// is what re-creating the query with new options did.
		let resubscribed = false;
		const disposeResubscribe = $effect.root(() => {
			const recreated = api.endpoints.query.useQuery(args, {
				subscriptionOptions: { pollingInterval: 300_000 },
			});
			$effect(() => {
				resubscribed = recreated.result !== undefined;
			});
		});
		flushSync();
		expect(resubscribed).toBe(true);
		await vi.waitFor(() => expect(baseQueryCalls).toHaveBeenCalledTimes(2));

		disposeResubscribe();
		dispose();
	});
});
