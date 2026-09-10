/** @vitest-environment jsdom */
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, createRef, type ReactNode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { assert } from "#ui/assert.ts";
import { Section } from "./Section.tsx";
import type { Plan } from "./layout.ts";
import type { Graph } from "./usePlan.ts";

globalThis.IS_REACT_ACT_ENVIRONMENT = true;
vi.mock("#ui/hotkeys.ts", () => ({
	workspaceHotkeys: { fetchFromRemotes: { hotkey: "F", meta: { name: "Fetch" } } },
}));
vi.mock("#ui/components/Kbd.tsx", () => ({ Kbd: () => null }));
vi.mock("#ui/components/Tooltip.tsx", () => ({
	TooltipPopup: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}));
vi.mock("#ui/api/mutations.ts", () => ({
	useWorkspaceIntegrateUpstream: () => ({ isPending: false, mutate: vi.fn() }),
}));
vi.mock("#ui/projects/state.ts", () => ({
	projectSlice: { selectors: { selectPendingOperation: () => ({ _tag: "None" }) } },
}));
vi.mock("#ui/store.ts", () => ({
	useAppSelector: (select: (state: object) => unknown) => select({}),
}));
vi.mock("#ui/routes/project/$id/workspace/WorkspaceLists/context.tsx", () => ({
	useAddressSpace: () => ({ items: [], indexByKey: new Map() }),
}));
vi.mock("#ui/routes/project/$id/workspace/useFetchFromRemotes.ts", () => ({
	useFetchFromRemotes: () => ({ fetch: vi.fn(), isPending: false, enabled: true }),
}));
vi.mock("./TargetCommitRow.tsx", () => ({ TargetCommitRow: () => null }));

let root: Root;
let container: HTMLDivElement;
let client: QueryClient;
const onMore = vi.fn();
const plan: Plan = {
	order: [],
	header: { label: "origin/main", current: false, incoming: 0 },
	incomingExpanded: false,
	incoming: [],
	historyAvailable: true,
	historyExpanded: true,
	history: [],
	historyHidden: 20,
	worktrees: { on: new Map(), standalone: [] },
};

beforeEach(() => {
	onMore.mockReset();
	client = new QueryClient({ defaultOptions: { queries: { retry: false, enabled: false } } });
	container = document.createElement("div");
	document.body.append(container);
	root = createRoot(container);
});
afterEach(async () => {
	await act(async () => root.unmount());
	client.clear();
	container.remove();
});
const render = async (historyMore: Graph["historyMore"] = "idle", current = false) => {
	await act(async () => {
		root.render(
			<QueryClientProvider client={client}>
				<Section
					projectId="section-test"
					plan={{ ...plan, header: { ...assert(plan.header), current } }}
					onToggleIncoming={vi.fn()}
					onToggleHistory={vi.fn()}
					onShowMoreHistory={onMore}
					historyMore={historyMore}
					onShowMoreRun={vi.fn()}
					onFoldRun={vi.fn()}
					scrollElementRef={createRef<HTMLDivElement>()}
				/>
			</QueryClientProvider>,
		);
	});
};
const more = () => assert(container.querySelector<HTMLElement>('[role="button"]'));

it("keeps the update action on a stale target with no incoming commits", async () => {
	await render();
	expect(container.textContent).toContain("Pull latest");
	expect(container.querySelector('[aria-label="Unfold incoming commits"]')).toBeNull();
	expect(container.textContent).not.toContain("Workspace base");
	await render("idle", true);
	expect(container.textContent).not.toContain("Pull latest");
});

it.each(["idle", "failed"] as const)(
	"makes the %s History action focusable and keyboard-activatable",
	async (state) => {
		await render(state);
		const button = more();
		expect(button.tabIndex).toBe(0);
		for (const key of ["Enter", " "]) {
			await act(async () => {
				button.focus();
				button.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true }));
				button.dispatchEvent(new KeyboardEvent("keyup", { key, bubbles: true }));
			});
		}
		expect(onMore).toHaveBeenCalledTimes(2);
	},
);

it("does not activate History pagination while a page is loading", async () => {
	await render("loading");
	const button = more();
	expect(button.getAttribute("aria-disabled")).toBe("true");
	await act(async () => {
		button.focus();
		button.click();
		button.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
	});
	expect(onMore).not.toHaveBeenCalled();
});
