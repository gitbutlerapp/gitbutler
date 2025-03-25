<!-- This is a V3 replacement for `BranchFileList.svelte` -->
<script lang="ts">
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import LazyloadContainer from '$components/LazyloadContainer.svelte';
	import FileListItemWrapper from '$components/v3/FileListItemWrapper.svelte';
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
		selectionId: CommitProps | BranchProps | WorktreeProps;
	};

	const { projectId, stackId, changes, selectionId }: Props = $props();

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

{#if visibleFiles.length > 0}
	<div class="file-list hide-native-scrollbar">
		<ScrollableContainer wide>
			<!-- Maximum amount for initial render is 100 files
	`minTriggerCount` set to 80 in order to start the loading a bit earlier. -->
			<LazyloadContainer
				minTriggerCount={80}
				ontrigger={() => {
					loadMore();
				}}
				role="listbox"
				onkeydown={handleKeyDown}
			>
				{#each visibleFiles as change, idx (change.path)}
					<FileListItemWrapper
						selectedFile={selectionId}
						{change}
						{projectId}
						{listActive}
						showCheckbox={showCheckboxes}
						selected={idSelection.has(change.path, selectionId)}
						onclick={(e) => {
							selectFilesInList(e, change, visibleFiles, idSelection, true, idx, selectionId);
						}}
					/>
				{/each}
			</LazyloadContainer>
		</ScrollableContainer>
	</div>
{/if}

<style lang="postcss">
	.file-list {
		flex-grow: 1;
		overflow: hidden;
	}
</style>
