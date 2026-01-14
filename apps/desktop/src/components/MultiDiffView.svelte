<!--
	MultiDiffView - A virtualized multi-file diff viewer

	This component renders a scrollable list of file diffs for commits, branches, or worktree changes.
	It uses VirtualList to efficiently handle large changesets by only rendering diffs that are
	currently visible in the viewport, significantly reducing memory usage and improving performance.

	@component
-->
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
		showBorder?: boolean;
		showRoundedEdges?: boolean;
		startIndex?: number;
	};

	let {
		projectId,
		stackId,
		files,
		draggable,
		selectable,
		showBorder = true,
		showRoundedEdges = true,
		startIndex
	}: Props = $props();

	const idSelection = inject(FILE_SELECTION_MANAGER);
	const diffService = inject(DIFF_SERVICE);

	let virtualList = $state<VirtualList<SelectedFile>>();
	let highlightedIndex = $state<number | null>(null);

	export function jumpToIndex(index: number) {
		virtualList?.jumpToIndex(index);
		highlightedIndex = index;
	}
</script>

<div class="multi-diff-view" class:no-border={!showBorder} class:no-rounded={!showRoundedEdges}>
	{#if files && files.length > 0}
		<VirtualList
			bind:this={virtualList}
			{startIndex}
			grow
			items={files}
			defaultHeight={200}
			visibility="scroll"
		>
			{#snippet template(file, index)}
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
							topBorder={index !== 0}
							filePath={change.path}
							fileStatus={computeChangeStatus(change)}
							linesAdded={patchData?.linesAdded}
							linesRemoved={patchData?.linesRemoved}
							executable={isExecutable}
							highlighted={highlightedIndex === index}
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
					{#snippet loading()}
						<div style="height: 200px">loading</div>
					{/snippet}
				</ReduxResult>
			{/snippet}
		</VirtualList>
	{:else}
		<FilePreviewPlaceholder />
	{/if}
</div>

<style>
	.multi-diff-view {
		display: flex;
		flex-grow: 1;
		width: 100%;
		height: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		&.no-border {
			border: none;
		}

		&.no-rounded {
			border-radius: 0;
		}
	}
</style>
