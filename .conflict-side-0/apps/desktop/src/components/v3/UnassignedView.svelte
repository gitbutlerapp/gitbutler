<script lang="ts">
	import WorktreeChanges from '$components/v3/WorktreeChanges.svelte';
	import WorktreeTipsFooter from '$components/v3/WorktreeTipsFooter.svelte';
	import noChanges from '$lib/assets/illustrations/no-changes.svg?raw';
	import { DefinedFocusable } from '$lib/focus/focusManager.svelte';
	import { UncommittedService } from '$lib/selection/uncommittedService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import type { SelectionId } from '$lib/selection/key';

	interface Props {
		projectId: string;
		focus: DefinedFocusable;
	}

	const { projectId, focus }: Props = $props();

	const selectionId = { type: 'worktree', stackId: undefined } as SelectionId;

	const [uiState, uncommittedService] = inject(UiState, UncommittedService);
	const projectState = $derived(uiState.project(projectId));
	const exclusiveAction = $derived(projectState.exclusiveAction.current);
	let isScrollable = $state<boolean>(false);

	const treeChanges = $derived(uncommittedService.changesByStackId(null));
	const changesToCommit = $derived(treeChanges.current.length > 0);
</script>

<div class="unassigned">
	<WorktreeChanges
		title="Unassigned"
		{projectId}
		stackId={undefined}
		active={selectionId.type === 'worktree' &&
			selectionId.stackId === undefined &&
			focus === DefinedFocusable.UncommittedChanges}
		onscrollexists={(exists: boolean) => {
			isScrollable = exists;
		}}
		overflow
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

	{#if (exclusiveAction?.type !== 'commit' && exclusiveAction?.stackId) || changesToCommit}
		<div class="create-new" class:sticked-bottom={isScrollable}>
			<Button
				type="button"
				wide
				onclick={() => {
					projectState.exclusiveAction.set({ type: 'commit' });
					uncommittedService.checkAll(null);
				}}
				icon="amend-commit"
				testId={TestId.CommitToNewBranchButton}
				kind="outline"
			>
				Commit to new branch
			</Button>
		</div>
	{/if}
</div>

<style lang="postcss">
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

	.unassigned {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
		/* gap: 12px; */
		background-color: var(--clr-bg-1);
	}

	.create-new {
		display: flex;
		padding: 12px 12px 14px 12px;

		background-color: var(--clr-bg-1);
	}

	/* MODIFIERS */
	.sticked-bottom {
		border-top: 1px solid var(--clr-border-2);
	}
</style>
