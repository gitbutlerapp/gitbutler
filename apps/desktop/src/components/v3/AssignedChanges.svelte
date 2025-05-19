<script lang="ts">
	import CardOverlay from '$components/CardOverlay.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import WorktreeChangesFileList from '$components/v3/WorktreeChangesFileList.svelte';
	import WorktreeChangesSelectAll from '$components/v3/WorktreeChangesSelectAll.svelte';
	import { assignedChangesFocusableId } from '$lib/focus/focusManager.svelte';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { DiffService } from '$lib/hunks/diffService.svelte';
	import { AssignmentDropHandler } from '$lib/hunks/dropHandler';
	import { combineResults } from '$lib/state/helpers';
	import { UiState } from '$lib/state/uiState.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';

	type Props = {
		projectId: string;
		stackId: string;
	};

	const { projectId, stackId }: Props = $props();

	const [worktreeService, diffService, uiState] = inject(WorktreeService, DiffService, UiState);
	const projectState = $derived(uiState.project(projectId));
	const drawer = $derived(projectState.drawerPage);
	const isCommitting = $derived(drawer.current === 'new-commit');

	const changesResult = $derived(worktreeService.changes(projectId));
	const assignmentResult = $derived(worktreeService.assignments(projectId));
</script>

<ReduxResult
	{projectId}
	{stackId}
	result={combineResults(changesResult.current, assignmentResult.current)}
>
	{#snippet children([changes, assignments], { stackId, projectId })}
		{@const assignmentDZHandler = new AssignmentDropHandler(
			projectId,
			diffService,
			assignments,
			stackId
		)}
		<Dropzone handlers={[assignmentDZHandler].filter(isDefined)}>
			{#snippet overlay({ hovered, activated })}
				<CardOverlay {hovered} {activated} />
			{/snippet}
			<div class="assigned-changes" use:focusable={{ id: assignedChangesFocusableId(stackId) }}>
				<div class="assigned-changes__title text-14 text-bold">
					{#if isCommitting}
						<WorktreeChangesSelectAll {changes} {assignments} {stackId} />
					{/if}
					assigned changes
				</div>
				<WorktreeChangesFileList {projectId} {stackId} listMode="list" active />
			</div>
		</Dropzone>
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.assigned-changes {
		margin: 12px 12px 0;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
	}

	.assigned-changes__title {
		display: flex;
		padding: 8px 14px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-2);
	}
</style>
