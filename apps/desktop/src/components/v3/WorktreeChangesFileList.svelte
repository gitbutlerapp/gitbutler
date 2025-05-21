<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import FileList from '$components/v3/FileList.svelte';
	import {
		DiffService,
		hunkGroupToKey,
		type HunkAssignments,
		type HunkGroup
	} from '$lib/hunks/diffService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import type { TreeChange } from '$lib/hunks/change';

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
	const changesResult = $derived(worktreeService.getChanges(projectId));
	const changesKeyResult = $derived(worktreeService.getChangesKey(projectId));
	const assignments = $derived(
		changesKeyResult.current
			? diffService.hunkAssignments(projectId, changesKeyResult.current)
			: undefined
	);

	const isCommitting = $derived(drawerPage.current === 'new-commit');

	function filter(changes: TreeChange[], assignments: HunkAssignments) {
		const stackGroup = assignments.get(hunkGroupToKey(group));

		if (!stackGroup) return [];

		const filteredChanges = [];
		for (const change of changes) {
			const pathGroup = stackGroup.get(change.path);
			if (pathGroup) {
				filteredChanges.push(change);
			}
		}

		return filteredChanges;
	}
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
						changes={filter(changes, assignments)}
						{listMode}
						{active}
						{group}
					/>
				{/snippet}
			</ReduxResult>
		{/if}
	{/snippet}
</ReduxResult>
