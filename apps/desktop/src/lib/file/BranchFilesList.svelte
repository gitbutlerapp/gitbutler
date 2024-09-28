<script lang="ts">
	import BranchFilesHeader from './BranchFilesHeader.svelte';
	import FileListItem from './FileListItem.svelte';
	import LazyloadContainer from '$lib/shared/LazyloadContainer.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';
	import { chunk } from '$lib/utils/array';
	import { copyToClipboard } from '$lib/utils/clipboard';
	import { getContext, maybeGetContextStore } from '$lib/utils/context';
	import { KeyName } from '$lib/utils/hotkeys';
	import { selectFilesInList } from '$lib/utils/selectFilesInList';
	import { updateSelection } from '$lib/utils/selection';
	import { getCommitStore } from '$lib/vbranches/contexts';
	import { FileIdSelection, stringifyFileKey } from '$lib/vbranches/fileIdSelection';
	import { sortLikeFileTree } from '$lib/vbranches/filetree';
	import { SelectedOwnership, updateOwnership } from '$lib/vbranches/ownership';
	import Button from '@gitbutler/ui/Button.svelte';
	import type { AnyFile } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	const MERGE_DIFF_COMMAND = 'git diff-tree --cc ';

	interface Props {
		files: AnyFile[];
		isUnapplied?: boolean;
		showCheckboxes?: boolean;
		allowMultiple?: boolean;
		readonly?: boolean;
		commitDialogExpanded?: Writable<boolean>;
		focusCommitDialog?: () => void;
	}

	const {
		files,
		isUnapplied = false,
		showCheckboxes = false,
		allowMultiple = false,
		readonly = false,
		commitDialogExpanded,
		focusCommitDialog
	}: Props = $props();

	const fileIdSelection = getContext(FileIdSelection);
	const selectedOwnership: Writable<SelectedOwnership> | undefined =
		maybeGetContextStore(SelectedOwnership);
	const commit = getCommitStore();

	let chunkedFiles: AnyFile[][] = $derived(chunk(sortLikeFileTree(files), 100));
	let currentDisplayIndex = $state(0);
	let displayedFiles: AnyFile[] = $derived(chunkedFiles.slice(0, currentDisplayIndex + 1).flat());

	function handleSpace() {
		if (commitDialogExpanded === undefined) return;

		if (!$commitDialogExpanded) {
			$commitDialogExpanded = true;
			return;
		}

		updateOwnership({
			selectedFileIds: $fileIdSelection,
			files: displayedFiles,
			selectedOwnership
		});
	}

	function handleEnter() {
		if (commitDialogExpanded === undefined || focusCommitDialog === undefined) return;
		if ($commitDialogExpanded) {
			focusCommitDialog();
		}
	}

	function handleKeyDown(e: KeyboardEvent) {
		e.preventDefault();
		updateSelection({
			allowMultiple,
			metaKey: e.metaKey,
			shiftKey: e.shiftKey,
			key: e.key,
			targetElement: e.currentTarget as HTMLElement,
			files: displayedFiles,
			selectedFileIds: $fileIdSelection,
			fileIdSelection,
			commitId: $commit?.id
		});

		switch (e.key) {
			case KeyName.Space: {
				handleSpace();
				break;
			}
			case KeyName.Enter: {
				handleEnter();
				break;
			}
		}
	}

	function loadMore() {
		if (currentDisplayIndex + 1 >= chunkedFiles.length) return;
		currentDisplayIndex += 1;
	}
</script>

{#if !$commit?.isMergeCommit()}
	<BranchFilesHeader title="Changed files" {files} {showCheckboxes} />
{:else}
	<div class="merge-commit-error">
		<p class="info">
			Displaying diffs for merge commits is currently not supported. Please view the merge commit in
			GitHub, or run the following command in your project directory:
		</p>
		<div class="command">
			<TextBox value={MERGE_DIFF_COMMAND + $commit.id.slice(0, 7)} wide readonly />
			<Button
				icon="copy"
				style="ghost"
				outline
				onmousedown={() => copyToClipboard(MERGE_DIFF_COMMAND + $commit.id.slice(0, 7))}
			/>
		</div>
	</div>
{/if}

{#if displayedFiles.length > 0}
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
		{#each displayedFiles as file (file.id)}
			<FileListItem
				{file}
				{readonly}
				{isUnapplied}
				showCheckbox={showCheckboxes}
				selected={$fileIdSelection.includes(stringifyFileKey(file.id, $commit?.id))}
				onclick={(e) => {
					selectFilesInList(e, file, fileIdSelection, displayedFiles, allowMultiple, $commit);
				}}
			/>
		{/each}
	</LazyloadContainer>
{/if}

<style lang="postcss">
	.merge-commit-error {
		display: flex;
		flex-direction: column;
		gap: 14px;
		padding: 14px;

		& .info {
			color: var(--clr-text-2);
		}

		& .command {
			display: flex;
			gap: 8px;
			align-items: center;
			width: 100%;
		}
	}
</style>
