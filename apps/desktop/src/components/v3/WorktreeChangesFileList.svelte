<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import FileList from '$components/v3/FileList.svelte';
	import { filterChangesByGroup } from '$lib/selection/changeSelection.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { getContext } from '@gitbutler/shared/context';

	// TODO: Look into whether it's important to pass the stackId through
	type Props = {
		projectId: string;
		listMode: 'list' | 'tree';
		active: boolean;
		stackId: string;
	};

	const { projectId, listMode, active, stackId }: Props = $props();

	const worktreeService = getContext(WorktreeService);
	const uiState = getContext(UiState);
	const projectState = $derived(uiState.project(projectId));
	const drawerPage = $derived(projectState.drawerPage.get());
	const isCommitting = $derived(drawerPage.current === 'new-commit');
	const changesResult = $derived(worktreeService.changes(projectId));
	const assignments = $derived(worktreeService.assignments(projectId));
</script>

<ReduxResult {projectId} {stackId} result={changesResult.current}>
	{#snippet children(changes, { projectId, stackId })}
		{#if assignments?.current}
			<ReduxResult {projectId} result={assignments.current}>
				{#snippet children(assignments, { projectId })}
					<FileList
						draggableFiles
						selectionId={{ type: 'worktree', stackId }}
						showCheckboxes={isCommitting}
						{projectId}
						changes={filterChangesByGroup(changes, stackId, assignments)}
						{listMode}
						{active}
					/>
				{/snippet}
			</ReduxResult>
		{/if}
	{/snippet}
</ReduxResult>
