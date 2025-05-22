<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import FileList from '$components/v3/FileList.svelte';
	import { DiffService, type HunkGroup } from '$lib/hunks/diffService.svelte';
	import { filterChangesByGroup } from '$lib/selection/changeSelection.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { getContext } from '@gitbutler/shared/context';

	// TODO: Look into whether it's important to pass the stackId through
	type Props = {
		projectId: string;
		listMode: 'list' | 'tree';
		active: boolean;
		group: HunkGroup;
	};

	const { projectId, listMode, active, group }: Props = $props();

	const worktreeService = getContext(WorktreeService);
	const diffService = getContext(DiffService);
	const uiState = getContext(UiState);
	const projectState = $derived(uiState.project(projectId));
	const drawerPage = $derived(projectState.drawerPage.get());
	const isCommitting = $derived(drawerPage.current === 'new-commit');
	const changesResult = $derived(worktreeService.getChanges(projectId));
	const changesKeyResult = $derived(worktreeService.getChangesKey(projectId));
	const assignments = $derived(
		changesKeyResult.current
			? diffService.hunkAssignments(projectId, changesKeyResult.current)
			: undefined
	);
</script>

<ReduxResult {projectId} result={changesResult.current}>
	{#snippet children(changes, { projectId })}
		{#if assignments?.current}
			<ReduxResult {projectId} result={assignments.current}>
				{#snippet children(assignments, { projectId })}
					<FileList
						selectionId={{ type: 'worktree', group }}
						showCheckboxes={isCommitting}
						{projectId}
						changes={filterChangesByGroup(changes, group, assignments)}
						{listMode}
						{active}
						{group}
					/>
				{/snippet}
			</ReduxResult>
		{/if}
	{/snippet}
</ReduxResult>
