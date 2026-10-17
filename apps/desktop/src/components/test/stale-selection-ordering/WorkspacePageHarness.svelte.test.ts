import WorkspacePageHarness from "$components/test/stale-selection-ordering/WorkspacePageHarness.svelte";
import { render } from "@testing-library/svelte";
import { tick } from "svelte";
import { VERSION } from "svelte/compiler";
import { describe, expect, test, vi } from "vitest";
import type { StackSelection } from "$lib/state/uiState.svelte";
import type { Commit, Segment, Stack } from "@gitbutler/but-sdk";

vi.mock("$app/navigation", () => ({ goto: vi.fn() }));
vi.mock("$app/state", () => ({
	page: {
		params: { projectId: "project-1" },
		url: new URL("https://example.com/project-1/workspace"),
	},
}));
vi.mock("$components/views/WorkspaceView.svelte", async () => ({
	default: (
		await import("$components/test/stale-selection-ordering/WorkspaceViewQueryBoundary.svelte")
	).default,
}));

const OLD_OID = "obsolete-oid";
const NEW_OID = "rewritten-oid";
const KEPT_OID = "kept-oid";

type SelectionStore = {
	readonly current: StackSelection | undefined;
	set(value: StackSelection | undefined): void;
};

function stackWith(ids: string[]): Stack {
	return {
		id: "stack-1",
		base: null,
		segments: [
			{
				refName: { displayName: "branch", fullNameBytes: [] },
				remoteTrackingRefName: null,
				commits: ids.map((id) => ({ id, state: { type: "LocalOnly" } }) as Commit),
				commitsOnRemote: [],
				commitsOutside: null,
				metadata: null,
				isEntrypoint: false,
				pushStatus: "nothingToPush",
				base: null,
			} as Segment,
		],
	} as Stack;
}

async function settle() {
	await tick();
}

function mount(stack: Stack) {
	const queries: string[] = [];
	let selection!: SelectionStore;
	const props = {
		initialSelection: {
			branchName: "branch",
			commitId: OLD_OID,
			previewOpen: true,
		},
		stack,
		onQuery: (commitId: string) => queries.push(commitId),
		onStore: (store: SelectionStore) => (selection = store),
	};
	return {
		...render(WorkspacePageHarness, { props }),
		props,
		queries,
		get selection() {
			return selection;
		},
	};
}

function schedulingEvidence(queries: string[]) {
	return `Svelte ${VERSION}; detail queries: ${JSON.stringify(queries)}`;
}

describe("workspace stale-selection ordering", () => {
	test("repairs an initial cached selection before querying commit details", async () => {
		const mounted = mount(stackWith([NEW_OID]));
		await settle();

		expect(mounted.queries, schedulingEvidence(mounted.queries)).not.toContain(OLD_OID);
		expect(mounted.selection.current).toEqual({ branchName: "branch", previewOpen: false });
	});

	test("queries only the replacement after a rewrite", async () => {
		const mounted = mount(stackWith([OLD_OID]));
		await settle();
		mounted.queries.length = 0;

		await mounted.rerender({ ...mounted.props, stack: stackWith([NEW_OID]) });
		await settle();

		expect(mounted.queries, schedulingEvidence(mounted.queries)).not.toContain(OLD_OID);
		expect(mounted.queries).toContain(NEW_OID);
		expect(mounted.selection.current?.commitId).toBe(NEW_OID);
	});

	test("clears a removed commit before querying details", async () => {
		const mounted = mount(stackWith([OLD_OID, KEPT_OID]));
		await settle();
		mounted.queries.length = 0;

		await mounted.rerender({ ...mounted.props, stack: stackWith([KEPT_OID]) });
		await settle();

		expect(mounted.queries, schedulingEvidence(mounted.queries)).not.toContain(OLD_OID);
		expect(mounted.selection.current).toEqual({ branchName: "branch", previewOpen: false });
	});

	test("keeps a caller-selected OID while a cached stack object is retained", async () => {
		const stack = stackWith([OLD_OID]);
		const mounted = mount(stack);
		await settle();
		mounted.queries.length = 0;

		mounted.selection.set({ branchName: "branch", commitId: NEW_OID, previewOpen: true });
		await mounted.rerender({ ...mounted.props, stack });
		await settle();

		expect(mounted.selection.current?.commitId).toBe(NEW_OID);
		expect(mounted.queries, schedulingEvidence(mounted.queries)).not.toContain(OLD_OID);
		expect(mounted.queries).toContain(NEW_OID);
	});

	test("tears down the owner and detail consumer together", async () => {
		const mounted = mount(stackWith([OLD_OID]));
		await settle();
		mounted.queries.length = 0;
		mounted.unmount();

		mounted.selection.set({ branchName: "branch", commitId: NEW_OID, previewOpen: true });
		await settle();

		expect(mounted.queries).toEqual([]);
	});
});
