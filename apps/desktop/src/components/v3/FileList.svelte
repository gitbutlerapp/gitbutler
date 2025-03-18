<!-- This is a V3 replacement for `BranchFileList.svelte` -->
<script lang="ts">
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import LazyloadContainer from '$components/LazyloadContainer.svelte';
	import FileListItemWrapper from '$components/v3/FileListItemWrapper.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { selectFilesInList, updateSelection } from '$lib/selection/idSelectionUtils';
	import { chunk } from '$lib/utils/array';
	import { sortLikeFileTree } from '$lib/worktree/changeTree';
	import { getContext } from '@gitbutler/shared/context';
	import type { TreeChange } from '$lib/hunks/change';

	interface BaseProps {
		type: 'commit' | 'branch' | 'worktree';
		changes: TreeChange[];
		projectId: string;
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
	}

	type Props = CommitProps | BranchProps | WorktreeProps;

	const props: Props = $props();

	let lazyloadContainer = $state<ReturnType<typeof LazyloadContainer>>();
	let currentDisplayIndex = $state(0);

	const fileChunks: TreeChange[][] = $derived(chunk(sortLikeFileTree(props.changes), 100));
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
			selectionParams: props,
			preventDefault: () => e.preventDefault()
		});
	}

	function loadMore() {
		if (currentDisplayIndex + 1 >= fileChunks.length) return;
		currentDisplayIndex += 1;
	}

	const projectId = $derived(props.projectId);
	const showCheckboxes = $derived(props.type === 'worktree' ? props.showCheckboxes : false);

	let containerFocused = $state(false);

	document.addEventListener('focusin', () => {
		containerFocused = !!lazyloadContainer?.hasFocus();
	});
</script>

{#if visibleFiles.length > 0}
	<div class="file-list hide-native-scrollbar">
		<ScrollableContainer wide>
			<!-- Maximum amount for initial render is 100 files
	`minTriggerCount` set to 80 in order to start the loading a bit earlier. -->
			<LazyloadContainer
				bind:this={lazyloadContainer}
				minTriggerCount={80}
				ontrigger={() => {
					loadMore();
				}}
				role="listbox"
				onkeydown={handleKeyDown}
			>
				{#each visibleFiles as change, idx (change.path)}
					<FileListItemWrapper
						index={idx}
						selectedFile={props}
						{containerFocused}
						{change}
						{projectId}
						showCheckbox={showCheckboxes}
						selected={idSelection.has(change.path, props)}
						onclick={(e) => {
							selectFilesInList(e, change, visibleFiles, idSelection, true, idx, props);
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
