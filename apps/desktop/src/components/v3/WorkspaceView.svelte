<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import Feed from '$components/v3/Feed.svelte';
	import MainViewport from '$components/v3/MainViewport.svelte';
	import MultiStackView from '$components/v3/MultiStackView.svelte';
	import SelectionView from '$components/v3/SelectionView.svelte';
	import UnassignedView from '$components/v3/UnassignedView.svelte';
	import { SettingsService } from '$lib/config/appSettingsV2';
	import {
		DefinedFocusable,
		FocusManager,
		parseFocusableId,
		stackFocusableId,
		uncommittedFocusableId
	} from '$lib/focus/focusManager.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState, type ExclusiveAction } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { setContext } from 'svelte';
	import { writable } from 'svelte/store';
	import type { SelectionId } from '$lib/selection/key';

	interface Props {
		projectId: string;
		stackId?: string;
	}

	const { stackId, projectId }: Props = $props();

	const [stackService, focusManager, idSelection, uiState, settingsService] = inject(
		StackService,
		FocusManager,
		IdSelection,
		UiState,
		SettingsService
	);
	const worktreeSelection = $derived(idSelection.getById({ type: 'worktree' }));
	const stacksResult = $derived(stackService.stacks(projectId));
	const projectState = $derived(uiState.project(projectId));
	const settingsStore = $derived(settingsService.appSettings);
	const canUseActions = $derived($settingsStore?.featureFlags.actions ?? false);
	const showingActions = $derived(projectState.showActions.current && canUseActions);
	const exclusiveAction = $derived(projectState.exclusiveAction.current);

	const snapshotFocusables = writable<string[]>([]);
	setContext('snapshot-focusables', snapshotFocusables);

	const stackFocusables = $derived(
		stacksResult.current?.data
			? stacksResult.current.data.map((stack) => stackFocusableId(stack.id))
			: []
	);

	const uncommittedFocusables = $derived(
		stacksResult.current?.data
			? stacksResult.current.data.map((stack) => uncommittedFocusableId(stack.id))
			: []
	);

	let focusGroup = $derived(
		focusManager.radioGroup({
			triggers: [
				DefinedFocusable.UncommittedChanges,
				DefinedFocusable.Drawer,
				...stackFocusables,
				...$snapshotFocusables,
				...uncommittedFocusables
			]
		})
	);

	const focusedStackId = $derived(
		focusGroup.current ? parseFocusableId(focusGroup.current) : undefined
	);

	const selectionId = { type: 'worktree', stackId: undefined } as SelectionId;

	const lastAdded = $derived(worktreeSelection.lastAdded);
	const previewOpen = $derived(!!$lastAdded?.key);

	// Ensures that the exclusive action is still valid.
	$effect(() => {
		if (exclusiveAction?.type === 'commit') {
			ensureExclusiveCommitValid(exclusiveAction);
		}
	});

	function ensureExclusiveCommitValid(action: ExclusiveAction & { type: 'commit' }) {
		// We are committing to a stack that has not been created yet
		if (!action.stackId) {
			return;
		}

		const stacks = stackService.stacks(projectId);
		const branch = stackService.branchDetails(projectId, action.stackId, action.branchName);

		$effect(() => {
			const stackFound = stacks.current?.data?.find((s) => s.id === action.stackId);
			if (!stackFound) {
				uiState.project(projectId).exclusiveAction.set(undefined);
			}

			if (!action.branchName) {
				return;
			}

			if (!branch?.current?.data) {
				uiState.project(projectId).exclusiveAction.set(undefined);
				return;
			}

			// If the parentCommitId is not set, we are committing to the top of the stack.
			if (!action.parentCommitId) {
				return;
			}

			const hasCommit = branch.current.data.commits.some((c) => c.id === action.parentCommitId);
			if (!hasCommit) {
				uiState.project(projectId).exclusiveAction.set(undefined);
			}
		});
	}
</script>

{#snippet right()}
	<Feed {projectId} onCloseClick={() => uiState.project(projectId).showActions.set(false)} />
{/snippet}

{#snippet leftPreview()}
	<SelectionView {projectId} {selectionId} draggableFiles />
{/snippet}

<MainViewport
	name="workspace"
	leftWidth={{ default: 280, min: 220 }}
	preview={previewOpen ? leftPreview : undefined}
	previewWidth={{ default: 480, min: 220 }}
	right={showingActions ? right : undefined}
	rightWidth={{ default: 320, min: 220 }}
>
	{#snippet left()}
		<UnassignedView {projectId} focus={focusGroup.current as DefinedFocusable} />
	{/snippet}
	{#snippet middle()}
		<ReduxResult {projectId} result={stacksResult?.current}>
			{#snippet loading()}
				<div class="stacks-view-skeleton"></div>
			{/snippet}
			{#snippet children(stacks, { projectId })}
				<MultiStackView {projectId} {stacks} {selectionId} selectedId={stackId} {focusedStackId} />
			{/snippet}
		</ReduxResult>
	{/snippet}
</MainViewport>

<style>
	.stacks-view-skeleton {
		width: 100%;
		height: 100%;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}
</style>
