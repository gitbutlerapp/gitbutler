<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import MainViewport from '$components/v3/MainViewport.svelte';
	import MultiStackView from '$components/v3/MultiStackView.svelte';
	import SelectionView from '$components/v3/SelectionView.svelte';
	import WorktreeChanges from '$components/v3/WorktreeChanges.svelte';
	import WorktreeTipsFooter from '$components/v3/WorktreeTipsFooter.svelte';
	import noChanges from '$lib/assets/illustrations/no-changes.svg?raw';
	import {
		DefinedFocusable,
		FocusManager,
		parseFocusableId,
		stackFocusableId,
		uncommittedFocusableId
	} from '$lib/focus/focusManager.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { setContext } from 'svelte';
	import { writable } from 'svelte/store';
	import type { SelectionId } from '$lib/selection/key';

	interface Props {
		projectId: string;
		stackId?: string;
	}

	const { stackId, projectId }: Props = $props();

	const [stackService, focusManager, idSelection] = inject(StackService, FocusManager, IdSelection);
	const worktreeSelection = idSelection.getById({ type: 'worktree' });
	const stacksResult = $derived(stackService.stacks(projectId));

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
</script>

<MainViewport
	name="workspace"
	middleOpen={previewOpen}
	leftWidth={{ default: 280, min: 240 }}
	middleWidth={{ default: 380, min: 240 }}
>
	{#snippet left()}
		<WorktreeChanges
			title="Unassigned"
			{projectId}
			stackId={undefined}
			active={selectionId.type === 'worktree' &&
				selectionId.stackId === undefined &&
				focusGroup.current === DefinedFocusable.UncommittedChanges}
		>
			{#snippet emptyPlaceholder()}
				<div class="unassigned-changes__empty">
					<div class="unassigned-changes__empty__placeholder">
						{@html noChanges}
						<p class="text-13 text-body unassigned-changes__empty__placeholder-text">
							You're all caught up!<br />
							No files need committing
						</p>
					</div>
					<WorktreeTipsFooter />
				</div>
			{/snippet}
		</WorktreeChanges>
	{/snippet}
	{#snippet middle()}
		<SelectionView {projectId} {selectionId} draggableFiles />
	{/snippet}
	{#snippet right()}
		<ReduxResult {projectId} result={stacksResult?.current}>
			{#snippet loading()}
				<div class="stacks-view-skeleton"></div>
			{/snippet}

			{#snippet children(stacks)}
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

	.unassigned-changes__empty {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.unassigned-changes__empty__placeholder {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 0 20px 40px;
		gap: 20px;
	}

	.unassigned-changes__empty__placeholder-text {
		color: var(--clr-text-3);
		text-align: center;
	}
</style>
