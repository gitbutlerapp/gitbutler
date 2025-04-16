<!-- This is a V3 replacement for `BranchFileList.svelte` -->
<script lang="ts">
	import LazyloadContainer from '$components/LazyloadContainer.svelte';
	import FileListItemWrapper from '$components/v3/FileListItemWrapper.svelte';
	import FileTreeNode from '$components/v3/FileTreeNode.svelte';
	import { abbreviateFolders, changesToFileTree } from '$lib/files/filetreeV3';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { selectFilesInList, updateSelection } from '$lib/selection/idSelectionUtils';
	import { chunk } from '$lib/utils/array';
	import { sortLikeFileTree } from '$lib/worktree/changeTree';
	import { getContext } from '@gitbutler/shared/context';
	import type { TreeChange } from '$lib/hunks/change';
	import type { SelectionId } from '$lib/selection/key';

	type Props = {
		projectId: string;
		stackId?: string;
		changes: TreeChange[];
		listMode: 'list' | 'tree';
		showCheckboxes?: boolean;
		selectionId: SelectionId;
		listActive: boolean;
	};

	const { projectId, changes, listMode, selectionId, showCheckboxes, listActive }: Props = $props();

	let currentDisplayIndex = $state(0);

	const fileChunks: TreeChange[][] = $derived(chunk(sortLikeFileTree(changes), 100));
	const visibleFiles: TreeChange[] = $derived(fileChunks.slice(0, currentDisplayIndex + 1).flat());
	const idSelection = getContext(IdSelection);

	function handleKeyDown(e: KeyboardEvent) {
		updateSelection({
			allowMultiple: true,
			metaKey: e.metaKey,
			shiftKey: e.shiftKey,
			key: e.key,
			targetElement: e.currentTarget as HTMLElement,
			files: visibleFiles,
			selectedFileIds: idSelection.values(selectionId),
			fileIdSelection: idSelection,
			selectionId: selectionId,
			preventDefault: () => e.preventDefault()
		});
	}

	function loadMore() {
		if (currentDisplayIndex + 1 >= fileChunks.length) return;
		currentDisplayIndex += 1;
	}
</script>

{#snippet fileTemplate(change: TreeChange, idx: number, depth: number = 0)}
	<FileListItemWrapper
		{selectionId}
		{change}
		{projectId}
		{listActive}
		{listMode}
		{depth}
		showCheckbox={showCheckboxes}
		isLast={idx === visibleFiles.length - 1}
		selected={idSelection.has(change.path, selectionId)}
		onclick={(e) => {
			selectFilesInList(e, change, visibleFiles, idSelection, true, idx, selectionId);
		}}
	/>
{/snippet}

{#if visibleFiles.length > 0}
	<LazyloadContainer
		minTriggerCount={80}
		ontrigger={() => {
			loadMore();
		}}
		role="listbox"
		onkeydown={handleKeyDown}
	>
		{#if listMode === 'tree'}
			{@const node = abbreviateFolders(changesToFileTree(changes))}
			<FileTreeNode isRoot {node} {showCheckboxes} {changes} {fileTemplate} />
		{:else}
			{#each visibleFiles as change, idx}
				{@render fileTemplate(change, idx)}
			{/each}
		{/if}
	</LazyloadContainer>
{/if}
