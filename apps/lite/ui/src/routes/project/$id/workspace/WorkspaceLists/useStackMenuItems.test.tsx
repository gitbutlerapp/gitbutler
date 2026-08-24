/** @vitest-environment jsdom */

import { operatingModeQueryOptions } from "#ui/api/queries.ts";
import type { NativeMenuItem } from "#ui/native-menu.ts";
import type { OperatingMode, Stack } from "@gitbutler/but-sdk";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, type FC } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

declare global {
	var IS_REACT_ACT_ENVIRONMENT: boolean | undefined;
}

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const hookState = vi.hoisted(() => ({ noOperationPending: true, unapplyPending: false }));

vi.mock("#ui/api/mutations.ts", () => ({
	useUnapplyStack: () => ({ isPending: hookState.unapplyPending, mutate: vi.fn() }),
	useWorkspaceIntegrateUpstream: () => ({ mutate: vi.fn() }),
}));
vi.mock("#ui/api/stack.ts", () => ({ stackBottomRelativeTo: () => null }));
vi.mock("#ui/hotkeys.ts", () => ({
	sidebarHotkeys: { updateStack: { hotkey: "Mod+R" } },
	toElectronAccelerator: () => "CommandOrControl+R",
}));
vi.mock("#ui/projects/state.ts", () => ({
	projectSlice: {
		actions: { setSegmentsFolded: vi.fn() },
		selectors: {
			selectPendingOperation: () => ({ _tag: hookState.noOperationPending ? "None" : "Pending" }),
			selectSegmentFolded: () => false,
		},
	},
}));
vi.mock("#ui/store.ts", () => ({
	useAppDispatch: () => vi.fn(),
	useAppSelector: (selector: (state: object) => unknown) => selector({}),
}));

import { useStackMenuItems } from "./useStackMenuItems.ts";

const projectId = "project-id";
const stack = { id: "stack-id", base: null, segments: [] } as Stack;

const Probe: FC = () => {
	const unapply = useStackMenuItems(projectId, stack).find(
		(item): item is Extract<NativeMenuItem, { _tag: "Item" }> =>
			item._tag === "Item" && item.label === "Unapply Whole Stack",
	);
	return <output data-enabled={String(unapply?.enabled)} />;
};

describe("useStackMenuItems", () => {
	let container: HTMLDivElement;
	let queryClient: QueryClient;
	let root: Root;

	beforeEach(() => {
		hookState.noOperationPending = true;
		hookState.unapplyPending = false;
		vi.stubGlobal("lite", { operatingMode: vi.fn(() => new Promise(() => {})) });
		container = document.createElement("div");
		document.body.append(container);
		queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
		root = createRoot(container);
	});

	afterEach(() => {
		act(() => root.unmount());
		queryClient.clear();
		container.remove();
		vi.unstubAllGlobals();
	});

	const enabled = () => container.querySelector("output")?.dataset.enabled === "true";
	const render = () =>
		act(() =>
			root.render(
				<QueryClientProvider client={queryClient}>
					<Probe />
				</QueryClientProvider>,
			),
		);
	const setOperatingMode = async (operatingMode: OperatingMode, expected: boolean) =>
		act(async () => {
			queryClient.setQueryData(operatingModeQueryOptions(projectId).queryKey, {
				head: null,
				operatingMode,
			});
			await vi.waitFor(() => expect(enabled()).toBe(expected));
		});

	it("enables unapply only in OpenWorkspace with no pending work", async () => {
		render();
		expect(enabled()).toBe(false);

		await setOperatingMode(
			{
				type: "OutsideWorkspace",
				subject: { branchName: "main", worktreeConflicts: [] },
			},
			false,
		);

		await setOperatingMode(
			{
				type: "Edit",
				subject: { commitOid: "commit", stackId: "stack-id" },
			},
			false,
		);

		await setOperatingMode({ type: "OpenWorkspace" }, true);

		hookState.noOperationPending = false;
		render();
		expect(enabled()).toBe(false);

		hookState.noOperationPending = true;
		hookState.unapplyPending = true;
		render();
		expect(enabled()).toBe(false);
	});
});
