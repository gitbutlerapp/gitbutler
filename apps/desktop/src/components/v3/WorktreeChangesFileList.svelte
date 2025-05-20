<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import FileList from '$components/v3/FileList.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { getContext } from '@gitbutler/shared/context';

	// TODO: Look into whether it's important to pass the stackId through
	type Props = {
		projectId: string;
		showCheckboxes: boolean;
		listMode: 'list' | 'tree';
		active: boolean;
	};

	const { projectId, showCheckboxes, listMode, active }: Props = $props();

	const worktreeService = getContext(WorktreeService);
	const changesResult = $derived(worktreeService.getChanges(projectId));
</script>

<ReduxResult {projectId} result={changesResult.current}>
	{#snippet children(changes, { projectId })}
		<FileList
			selectionId={{ type: 'worktree' }}
			{showCheckboxes}
			{projectId}
			{changes}
			{listMode}
			{active}
		/>
	{/snippet}
</ReduxResult>
