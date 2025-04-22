<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import FileListItemWrapper from '$components/v3/FileListItemWrapper.svelte';
	import UnifiedDiffView from '$components/v3/UnifiedDiffView.svelte';
	import { DiffService } from '$lib/hunks/diffService.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import type { SelectedFile } from '$lib/selection/key';

	type Props = {
		selectedFile: SelectedFile;
		projectId: string;
		onCloseClick: () => void;
	};

	const { selectedFile, projectId, onCloseClick }: Props = $props();

	const [diffService, stackService, worktreeService] = inject(
		DiffService,
		StackService,
		WorktreeService
	);

	const changeResult = $derived.by(() => {
		switch (selectedFile.type) {
			case 'commit':
				return stackService.commitChange(projectId, selectedFile.commitId, selectedFile.path);
			case 'branch':
				return stackService.branchChange({
					projectId,
					stackId: selectedFile.stackId,
					branchName: selectedFile.branchName,
					path: selectedFile.path
				});
			case 'worktree':
				return worktreeService.getChange(projectId, selectedFile.path);
		}
	});

	const change = $derived(changeResult.current.data);
	const diffResult = $derived(change ? diffService.getDiff(projectId, change) : undefined);
</script>

{#if diffResult?.current}
	<ReduxResult {projectId} result={combineResults(changeResult.current, diffResult.current)}>
		{#snippet children([change, diff], env)}
			<div class="selected-change-item">
				<FileListItemWrapper
					selectionId={selectedFile}
					projectId={env.projectId}
					{change}
					{diff}
					isHeader
					listMode="list"
					{onCloseClick}
				/>
				<UnifiedDiffView
					projectId={env.projectId}
					{change}
					{diff}
					selectable
					selectionId={selectedFile}
				/>
			</div>
		{/snippet}
	</ReduxResult>
{/if}

<style>
	.selected-change-item {
		display: flex;
		flex-direction: column;
		background-color: var(--clr-bg-1);
		border-bottom: 1px solid var(--clr-border-2);
	}
</style>
