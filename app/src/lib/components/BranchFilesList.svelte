<script lang="ts">
	import BranchFilesHeader from './BranchFilesHeader.svelte';
	import Button from './Button.svelte';
	import FileListItem from './FileListItem.svelte';
	import TextBox from '$lib/components/TextBox.svelte';
	import { copyToClipboard } from '$lib/utils/clipboard';
	import { getContext } from '$lib/utils/context';
	import { selectFilesInList } from '$lib/utils/selectFilesInList';
	import { maybeMoveSelection } from '$lib/utils/selection';
	import { getCommitStore } from '$lib/vbranches/contexts';
	import { FileIdSelection, stringifyFileKey } from '$lib/vbranches/fileIdSelection';
	import { sortLikeFileTree } from '$lib/vbranches/filetree';
	import type { AnyFile } from '$lib/vbranches/types';

	export let files: AnyFile[];
	export let isUnapplied = false;
	export let showCheckboxes = false;
	export let allowMultiple = false;
	export let readonly = false;

	const fileIdSelection = getContext(FileIdSelection);
	const commit = getCommitStore();

	function chunk<T>(arr: T[], size: number) {
		return Array.from({ length: Math.ceil(arr.length / size) }, (v, i) =>
			arr.slice(i * size, i * size + size)
		);
	}

	let chunkedFiles: AnyFile[][] = [];
	let displayedFiles: AnyFile[] = [];
	let currentDisplayIndex = 0;

	function setFiles(files: AnyFile[]) {
		chunkedFiles = chunk(sortLikeFileTree(files), 100);
		displayedFiles = chunkedFiles[0] || [];
		currentDisplayIndex = 0;
	}

	// Make sure we display when the file list is reset
	$: setFiles(files);

	export function loadMore() {
		if (currentDisplayIndex + 1 >= chunkedFiles.length) return;

		currentDisplayIndex += 1;
		displayedFiles = [...displayedFiles, ...chunkedFiles[currentDisplayIndex]];
	}
	let mergeDiffCommand = 'git diff-tree --cc ';
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
			<TextBox value={mergeDiffCommand + $commit.id.slice(0, 7)} wide readonly />
			<Button
				icon="copy"
				style="ghost"
				outline
				on:mousedown={() => copyToClipboard(mergeDiffCommand + $commit.id.slice(0, 7))}
			></Button>
		</div>
	</div>
{/if}

{#each displayedFiles as file (file.id)}
	<FileListItem
		{file}
		{readonly}
		{isUnapplied}
		showCheckbox={showCheckboxes}
		selected={$fileIdSelection.includes(stringifyFileKey(file.id, $commit?.id))}
		on:click={(e) => {
			selectFilesInList(e, file, fileIdSelection, displayedFiles, allowMultiple, $commit);
		}}
		on:keydown={(e) => {
			e.preventDefault();
			maybeMoveSelection(e.key, file, displayedFiles, fileIdSelection);
		}}
	/>
{/each}

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
