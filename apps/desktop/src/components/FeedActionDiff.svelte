<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import SnapshotAttachment from '$components/SnapshotAttachment.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { inject } from '@gitbutler/shared/context';
	import { flattenAndDeduplicate } from '@gitbutler/shared/utils/array';
	import { FileListItem } from '@gitbutler/ui';

	type Props = {
		projectId: string;
		newCommits: string[];
	};

	const { projectId, newCommits }: Props = $props();
	const stackService = inject(STACK_SERVICE);
	const changedFilesInCommits = $derived(
		stackService.filePathsChangedInCommits(projectId, newCommits)
	);
</script>

<ReduxResult {projectId} result={combineResults(...changedFilesInCommits.current)}>
	{#snippet children(filesPaths)}
		{@const dedupedFilePaths = flattenAndDeduplicate(filesPaths)}
		{#if dedupedFilePaths.length === 0}
			<p class="text-13 text-grey">No changes detected</p>
		{:else}
			<SnapshotAttachment
				foldable={dedupedFilePaths.length > 2}
				foldedAmount={dedupedFilePaths.length}
			>
				<div class="snapshot-files">
					{#each dedupedFilePaths as path, idx (path)}
						<FileListItem
							listMode="list"
							filePath={path}
							hideBorder={idx === dedupedFilePaths.length - 1}
						/>
					{/each}
				</div>
			</SnapshotAttachment>
		{/if}
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.snapshot-files {
		display: flex;
		flex-direction: column;
	}
</style>
