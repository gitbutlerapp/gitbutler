<script lang="ts">
	import WorkspacePage from "../../../routes/[projectId]/workspace/+page.svelte";
	import { MODE_SERVICE } from "$lib/mode/modeService";
	import { transformWorkspaceDetails } from "$lib/stacks/headInfoAdapters";
	import { STACK_SERVICE, StackService } from "$lib/stacks/stackService.svelte";
	import { UI_STATE, type StackSelection, type UiState } from "$lib/state/uiState.svelte";
	import { provide } from "@gitbutler/core/context";
	import type { BackendApi } from "$lib/state/backendApi";
	import type { RefInfo, Stack } from "@gitbutler/but-sdk";

	type SelectionStore = {
		readonly current: StackSelection | undefined;
		set(value: StackSelection | undefined): void;
	};

	type Props = {
		initialSelection: StackSelection;
		stack: Stack;
		onQuery: (commitId: string) => void;
		onStore: (store: SelectionStore) => void;
	};

	let { initialSelection, stack, onQuery, onStore }: Props = $props();
	function readInitialSelection() {
		return initialSelection;
	}
	let selection = $state<StackSelection | undefined>(readInitialSelection());
	const selectionStore: SelectionStore = {
		get current() {
			return selection;
		},
		set(value) {
			selection = value;
		},
	};
	const uiState = {
		lane: () => ({ selection: selectionStore }),
		project: () => ({ exclusiveAction: { current: undefined, set: () => {} } }),
	} as unknown as UiState;
	const workspaceDetails = $derived(transformWorkspaceDetails({ stacks: [stack] } as RefInfo));
	const backendApi = {
		endpoints: {
			workspaceDetails: {
				useQuery: (_args: unknown, options: { transform: (value: unknown) => unknown }) => ({
					get response() {
						return options.transform(workspaceDetails);
					},
				}),
			},
			commitDetails: {
				useQuery: (args: { commitId: string }) => {
					onQuery(args.commitId);
					return { result: { status: "pending" } };
				},
			},
		},
	} as unknown as BackendApi;
	const stackService = new StackService(backendApi, {} as never, uiState);

	provide(STACK_SERVICE, stackService);
	provide(UI_STATE, uiState);
	provide(MODE_SERVICE, { mode: () => ({ response: undefined }) } as never);
	function publishStore() {
		onStore(selectionStore);
	}
	publishStore();
</script>

<WorkspacePage />
