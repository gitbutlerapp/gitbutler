<script lang="ts">
	import FilePreviewPlaceholder from '$components/FilePreviewPlaceholder.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import UnifiedDiffView from '$components/UnifiedDiffView.svelte';
	import { isExecutableStatus } from '$lib/hunks/change';
	import { DIFF_SERVICE } from '$lib/hunks/diffService.svelte';
	import { FILE_SELECTION_MANAGER } from '$lib/selection/fileSelectionManager.svelte';
	import { type SelectedFile } from '$lib/selection/key';
	import { computeChangeStatus } from '$lib/utils/fileStatus';
	import { inject } from '@gitbutler/core/context';
	import { FileViewHeader, VirtualList } from '@gitbutler/ui';

	type Props = {
		projectId: string;
		stackId?: string;
		files?: SelectedFile[];
		draggable?: boolean;
		selectable: boolean;
	};

	let { projectId, stackId, files, draggable, selectable }: Props = $props();

	const idSelection = inject(FILE_SELECTION_MANAGER);
	const diffService = inject(DIFF_SERVICE);

	let virtualList = $state<VirtualList<SelectedFile>>();

	export function scrollToIndex(index: number) {
		virtualList?.scrollToIndex(index);
	}
</script>

<div class="selection-view">
	{#if files && files.length > 0}
		<VirtualList
			bind:this={virtualList}
			grow
			items={files}
			batchSize={1}
			defaultHeight={500}
			visibility="scroll"
		>
			{#snippet chunkTemplate(files)}
				{#each files as file, i}
					{@const changeQuery = idSelection.changeByKey(projectId, file)}
					<ReduxResult {projectId} result={changeQuery.result}>
						{#snippet children(change)}
							{@const diffQuery = diffService.getDiff(projectId, change)}
							{@const diffData = diffQuery.response}
							{@const isExecutable = isExecutableStatus(change.status)}
							{@const patchData = diffData?.type === 'Patch' ? diffData.subject : null}
							<FileViewHeader
								solid
								bottomBorder
								topBorder={i !== 0}
								filePath={change.path}
								fileStatus={computeChangeStatus(change)}
								linesAdded={patchData?.linesAdded}
								linesRemoved={patchData?.linesRemoved}
								executable={isExecutable}
								sticky
							/>
							<UnifiedDiffView
								{projectId}
								{stackId}
								commitId={file.type === 'commit' ? file.commitId : undefined}
								{draggable}
								{change}
								diff={diffData || null}
								{selectable}
								selectionId={file}
								topPadding
							/>
						{/snippet}
					</ReduxResult>
				{/each}
			{/snippet}
		</VirtualList>
	{:else}
		<FilePreviewPlaceholder />
	{/if}
</div>

<style>
	.selection-view {
		display: flex;
		flex-grow: 1;
		width: 100%;
		height: 100%;
		margin-top: 12px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}
</style>
