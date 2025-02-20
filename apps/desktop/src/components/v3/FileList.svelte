<!-- This is a V3 replacement for `BranchFileList.svelte` -->
<script lang="ts">
	import LazyloadContainer from '$components/LazyloadContainer.svelte';
	import ScrollableContainer from '$components/ScrollableContainer.svelte';
	import FileListItemWrapper from '$components/v3/FileListItemWrapper.svelte';
	import UnifiedDiffView from '$components/v3/UnifiedDiffView.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { selectFilesInList, updateSelection } from '$lib/selection/idSelectionUtils';
	import { chunk } from '$lib/utils/array';
	import { sortLikeFileTree } from '$lib/worktree/changeTree';
	import { getContext } from '@gitbutler/shared/context';
	import type { TreeChange } from '$lib/hunks/change';

	interface Props {
		changes: TreeChange[];
		projectId: string;
		/** The commit ID these changes belong to, if any. */
		commitId?: string;
		showCheckboxes?: boolean;
	}

	const { changes: files, projectId, commitId, showCheckboxes }: Props = $props();

	let currentDisplayIndex = $state(0);

	const fileChunks: TreeChange[][] = $derived(chunk(sortLikeFileTree(files), 100));
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
			selectedFileIds: idSelection.values(),
			fileIdSelection: idSelection,
			commitId,
			preventDefault: () => e.preventDefault()
		});
	}

	function loadMore() {
		if (currentDisplayIndex + 1 >= fileChunks.length) return;
		currentDisplayIndex += 1;
	}
</script>

{#if visibleFiles.length > 0}
	<div class="file-list hide-native-scrollbar">
		<ScrollableContainer wide>
			<!-- Maximum amount for initial render is 100 files
	`minTriggerCount` set to 80 in order to start the loading a bit earlier. -->
			<LazyloadContainer
				minTriggerCount={80}
				ontrigger={() => {
					console.log('loading more files...');
					loadMore();
				}}
				role="listbox"
				onkeydown={handleKeyDown}
			>
				{#each visibleFiles as change (change.path)}
					<FileListItemWrapper
						{change}
						{projectId}
						showCheckbox={showCheckboxes}
						selected={idSelection.has(change.path, commitId)}
						onclick={(e) => {
							selectFilesInList(e, change, visibleFiles, idSelection, true, commitId);
						}}
					>
						<UnifiedDiffView {projectId} {change} selectable />
					</FileListItemWrapper>
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
