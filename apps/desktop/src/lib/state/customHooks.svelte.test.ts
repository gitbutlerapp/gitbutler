import { butlerModule } from "$lib/state/butlerModule";
import { ReduxTag } from "$lib/state/tags";
import { configureStore } from "@reduxjs/toolkit";
import { buildCreateApi, coreModule } from "@reduxjs/toolkit/query";
import { describe, expect, test, vi } from "vitest";
import type { TauriBaseQueryFn } from "$lib/state/backendQuery";
import type { HookContext } from "$lib/state/context";
import type { PostHogWrapper } from "$lib/telemetry/posthog";

function setup() {
	const capture = vi.fn();
	const ctx: HookContext = {
		getState: () => store.getState(),
		getDispatch: () => store.dispatch,
		posthog: { capture } as unknown as PostHogWrapper,
	};
	async function baseQuery(
		args: Parameters<TauriBaseQueryFn>[0],
		_api: Parameters<TauriBaseQueryFn>[1],
		_extraOptions: Parameters<TauriBaseQueryFn>[2],
	): Promise<Awaited<ReturnType<TauriBaseQueryFn>>> {
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
		}),
	});
	const store = configureStore({
		reducer: { [api.reducerPath]: api.reducer },
		middleware: (getDefaultMiddleware) => getDefaultMiddleware().concat(api.middleware),
	});
	return { api, capture };
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
