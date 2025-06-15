<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import WorktreeChanges from '$components/v3/WorktreeChanges.svelte';
	import WorktreeTipsFooter from '$components/v3/WorktreeTipsFooter.svelte';
	import noChanges from '$lib/assets/illustrations/no-changes.svg?raw';
	import { DefinedFocusable } from '$lib/focus/focusManager.svelte';
	import { UncommittedService } from '$lib/selection/uncommittedService.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
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

	const [stackService, uncommittedService, uiState] = inject(
		StackService,
		UncommittedService,
		UiState
	);
	const projectState = $derived(uiState.project(projectId));
	const stacksResult = $derived(stackService.stacks(projectId));
	const selectionId = { type: 'worktree', stackId: undefined } as SelectionId;
	const isCommitting = $derived(projectState.exclusiveAction.current?.type === 'commit');
</script>

<div class="unassigned">
	<WorktreeChanges
		title="Unassigned"
		{projectId}
		stackId={undefined}
		active={selectionId.type === 'worktree' &&
			selectionId.stackId === undefined &&
			focus === DefinedFocusable.UncommittedChanges}
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

	<ReduxResult {projectId} result={stacksResult?.current}>
		{#snippet children(stacks)}
			{#if stacks.length === 0}
				<div class="start-commit">
					<Button
						testId={TestId.StartCommitButton}
						type="button"
						wide
						kind={isCommitting ? 'outline' : 'solid'}
						onclick={() => {
							if (isCommitting) {
								projectState.exclusiveAction.set(undefined);
							} else {
								projectState.exclusiveAction.set({ type: 'commit' });
								uncommittedService.checkAll(null);
							}
						}}
					>
						{#if isCommitting}
							Cancel
						{:else}
							Start a commit…
						{/if}
					</Button>
				</div>
			{/if}
		{/snippet}
	</ReduxResult>
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
		gap: 12px;
		background-color: var(--clr-bg-1);
	}
	.start-commit {
		padding: 12px;
	}
</style>
