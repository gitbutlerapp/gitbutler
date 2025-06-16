<script lang="ts">
	import WorktreeChanges from '$components/v3/WorktreeChanges.svelte';
	import WorktreeTipsFooter from '$components/v3/WorktreeTipsFooter.svelte';
	import noChanges from '$lib/assets/illustrations/no-changes.svg?raw';
	import { DefinedFocusable } from '$lib/focus/focusManager.svelte';
	import type { SelectionId } from '$lib/selection/key';

	interface Props {
		projectId: string;
		focus: DefinedFocusable;
	}

	const { projectId, focus }: Props = $props();

	const selectionId = { type: 'worktree', stackId: undefined } as SelectionId;
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
		gap: 12px;
		background-color: var(--clr-bg-1);
	}
</style>
