<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import FileListItemWrapper from '$components/v3/FileListItemWrapper.svelte';
	import UnifiedDiffView from '$components/v3/UnifiedDiffView.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import type { SelectedFile } from '$lib/selection/key';

	type Props = {
		selectedFile: SelectedFile;
		projectId: string;
		onCloseClick: () => void;
	};

	const { selectedFile, projectId, onCloseClick }: Props = $props();

	const [stackService, worktreeService] = inject(StackService, WorktreeService);

	const changeResult = $derived.by(() => {
		switch (selectedFile.type) {
			case 'commit':
				return stackService.commitChange(projectId, selectedFile.commitId, selectedFile.path);
			case 'branch':
				return stackService.branchChange(
					projectId,
					selectedFile.stackId,
					selectedFile.branchName,
					selectedFile.path
				);
			case 'worktree':
				return worktreeService.getChange(projectId, selectedFile.path);
		}
	});
</script>

<ReduxResult {projectId} result={changeResult.current}>
	{#snippet children(change, env)}
		<div class="selected-change-item">
			<FileListItemWrapper
				selectionId={selectedFile}
				projectId={env.projectId}
				{change}
				isHeader
				listMode="list"
				{onCloseClick}
			/>
			<UnifiedDiffView projectId={env.projectId} {change} selectable selectionId={selectedFile} />
		</div>
	{/snippet}
</ReduxResult>

<style>
	.selected-change-item {
		display: flex;
		flex-direction: column;
		background-color: var(--clr-bg-1);
		border-bottom: 1px solid var(--clr-border-2);
	}
</style>
