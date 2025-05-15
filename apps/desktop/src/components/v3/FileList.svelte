<!-- This is a V3 replacement for `BranchFileList.svelte` -->
<script lang="ts">
	import LazyloadContainer from '$components/LazyloadContainer.svelte';
	import FileListItemWrapper from '$components/v3/FileListItemWrapper.svelte';
	import FileTreeNode from '$components/v3/FileTreeNode.svelte';
	import { conflictEntryHint } from '$lib/conflictEntryPresence';
	import { abbreviateFolders, changesToFileTree } from '$lib/files/filetreeV3';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { selectFilesInList, updateSelection } from '$lib/selection/idSelectionUtils';
	import { type SelectionId } from '$lib/selection/key';
	import { chunk } from '$lib/utils/array';
	import { sortLikeFileTree } from '$lib/worktree/changeTree';
	import { getContext } from '@gitbutler/shared/context';
	import FileListItemV3 from '@gitbutler/ui/file/FileListItemV3.svelte';
	import type { ConflictEntriesObj } from '$lib/files/conflicts';
	import type { TreeChange, Modification } from '$lib/hunks/change';

	type Props = {
		projectId: string;
		stackId?: string;
		changes: TreeChange[];
		listMode: 'list' | 'tree';
		showCheckboxes?: boolean;
		selectionId: SelectionId;
		active?: boolean;
		conflictEntries?: ConflictEntriesObj;
	};

	const {
		projectId,
		changes,
		listMode,
		selectionId,
		showCheckboxes,
		active,
		stackId,
		conflictEntries
	}: Props = $props();

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

	const unrepresentedConflictedEntries = $derived.by(() => {
		if (!conflictEntries?.entries) return {};

		return Object.fromEntries(
			Object.entries(conflictEntries.entries).filter(([key, _value]) =>
				changes.every((change) => change.path !== key)
			)
		);
	});
</script>

{#snippet fileTemplate(change: TreeChange, idx: number, depth: number = 0)}
	{@const isExecutable = (change.status.subject as Modification).flags}
	<FileListItemWrapper
		{selectionId}
		{change}
		allChanges={changes}
		{projectId}
		{stackId}
		{active}
		{listMode}
		{depth}
		executable={!!isExecutable}
		showCheckbox={showCheckboxes}
		isLast={idx === visibleFiles.length - 1}
		selected={idSelection.has(change.path, selectionId)}
		onclick={(e) => {
			selectFilesInList(e, change, visibleFiles, idSelection, true, idx, selectionId);
		}}
		{conflictEntries}
	/>
{/snippet}

{#each Object.entries(unrepresentedConflictedEntries) as [path, kind]}
	<FileListItemV3
		filePath={path}
		conflicted
		conflictHint={conflictEntryHint(kind)}
		listMode="list"
	/>
{/each}
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
