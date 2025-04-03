<!-- This is a V3 replacement for `BranchFileList.svelte` -->
<script lang="ts">
	import LazyloadContainer from '$components/LazyloadContainer.svelte';
	import FileListItemWrapper from '$components/v3/FileListItemWrapper.svelte';
	import FileTreeNode from '$components/v3/FileTreeNode.svelte';
	import { abbreviateFolders, changesToFileTree } from '$lib/files/filetreeV3';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { selectFilesInList, updateSelection } from '$lib/selection/idSelectionUtils';
	import { UiState } from '$lib/state/uiState.svelte';
	import { chunk } from '$lib/utils/array';
	import { sortLikeFileTree } from '$lib/worktree/changeTree';
	import { getContext, inject } from '@gitbutler/shared/context';
	import type { TreeChange } from '$lib/hunks/change';

	interface BaseProps {
		type: 'commit' | 'branch' | 'worktree';
	}

	interface CommitProps extends BaseProps {
		type: 'commit';
		commitId: string;
	}

	interface BranchProps extends BaseProps {
		type: 'branch';
		stackId: string;
		branchName: string;
	}

	interface WorktreeProps extends BaseProps {
		type: 'worktree';
		showCheckboxes: boolean;
		stackId?: string;
	}

	type Props = {
		projectId: string;
		stackId?: string;
		changes: TreeChange[];
		listMode: 'list' | 'tree';
		selectionId: CommitProps | BranchProps | WorktreeProps;
	};

	const { projectId, stackId, changes, listMode, selectionId }: Props = $props();

	const [uiState] = inject(UiState);
	const stackState = $derived(stackId ? uiState.stack(stackId) : undefined);
	const activeSelection = $derived(stackState?.activeSelectionId.get());
	const listActive = $derived(activeSelection?.current.type === selectionId.type);

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

	const showCheckboxes = $derived(
		selectionId.type === 'worktree' ? selectionId.showCheckboxes : false
	);
</script>

{#snippet fileWrapper(change: TreeChange, idx: number, depth: number = 0)}
	<FileListItemWrapper
		selectedFile={selectionId}
		{change}
		{projectId}
		{listActive}
		{listMode}
		{depth}
		isLast={idx === visibleFiles.length - 1}
		selected={idSelection.has(change.path, selectionId)}
		onclick={(e) => {
			stackState?.activeSelectionId.set(selectionId);
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
			<FileTreeNode
				isRoot
				{stackId}
				{node}
				{showCheckboxes}
				{changes}
				{fileWrapper}
				onFolderClick={() => {
					console.warn('implement folder click to select all children');
				}}
			/>
		{:else}
			{#each visibleFiles as change, idx (change.path)}
				{@render fileWrapper(change, idx)}
			{/each}
		{/if}
	</LazyloadContainer>
{/if}
