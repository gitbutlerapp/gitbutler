/** @vitest-environment jsdom */

import { useWorkspaceIntegrateUpstream } from "#ui/api/mutations.ts";
import {
	olderTargetCommitsInfiniteQueryOptions,
	workspaceTargetCommitsQueryOptions,
} from "#ui/api/queries.ts";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("#ui/store.ts", () => ({ useAppDispatch: () => vi.fn() }));
vi.mock("#ui/use-cursor.ts", () => ({}));
vi.mock("@base-ui/react", () => ({ Toast: { useToastManager: () => ({ add: vi.fn() }) } }));

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const baseKey = workspaceTargetCommitsQueryOptions("project").queryKey;
const olderKey = olderTargetCommitsInfiniteQueryOptions("project", "cursor").queryKey;
const otherKey = workspaceTargetCommitsQueryOptions("other-project").queryKey;
const original = { commits: [], hasMore: true };
const updated = { commits: [], hasMore: false };

describe("useWorkspaceIntegrateUpstream", () => {
	let client: QueryClient;
	let root: Root;
	let integrate: ReturnType<typeof useWorkspaceIntegrateUpstream>["mutateAsync"];
	const workspaceIntegrateUpstream = vi.fn();

	beforeEach(() => {
		vi.stubGlobal("lite", { workspaceIntegrateUpstream });
		client = new QueryClient();
		client.setQueryData(baseKey, original);
		client.setQueryData(olderKey, { pages: [original], pageParams: ["cursor"] });
		client.setQueryData(otherKey, original);
		const container = document.createElement("div");
		root = createRoot(container);
		const Probe = () => {
			const mutation = useWorkspaceIntegrateUpstream();
			return (
				<button
					type="button"
					onClick={() => {
						integrate = mutation.mutateAsync;
					}}
				>
					Connect
				</button>
			);
		};
		act(() =>
			root.render(
				<QueryClientProvider client={client}>
					<Probe />
				</QueryClientProvider>,
			),
		);
		act(() => container.querySelector("button")?.click());
	});

	afterEach(() => {
		act(() => root.unmount());
		client.clear();
		vi.unstubAllGlobals();
	});

	const run = async (targetCommits: typeof updated | null | undefined, dryRun = false) => {
		workspaceIntegrateUpstream.mockResolvedValue({ workspace: null, targetCommits });
		await act(async () => {
			await integrate({ projectId: "project", updates: [], dryRun });
		});
	};

	it("seeds the base listing and invalidates older pages without invalidating the base", async () => {
		await run(updated);
		expect(client.getQueryData(baseKey)).toEqual(updated);
		expect(client.getQueryState(baseKey)?.isInvalidated).toBe(false);
		expect(client.getQueryState(olderKey)?.isInvalidated).toBe(true);
		expect(client.getQueryState(otherKey)?.isInvalidated).toBe(false);
	});

	it.each([null, undefined])(
		"invalidates all target pages when the listing is %s",
		async (listing) => {
			await run(listing);
			expect(client.getQueryData(baseKey)).toEqual(original);
			expect(client.getQueryState(baseKey)?.isInvalidated).toBe(true);
			expect(client.getQueryState(olderKey)?.isInvalidated).toBe(true);
			expect(client.getQueryState(otherKey)?.isInvalidated).toBe(false);
		},
	);

	it("leaves cached listings untouched for a dry run", async () => {
		await run(updated, true);
		expect(client.getQueryData(baseKey)).toEqual(original);
		expect(client.getQueryState(baseKey)?.isInvalidated).toBe(false);
		expect(client.getQueryState(olderKey)?.isInvalidated).toBe(false);
	});
});
