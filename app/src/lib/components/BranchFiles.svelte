<script lang="ts">
	import BranchFilesList from './BranchFilesList.svelte';
	import { getContext } from '$lib/utils/context';
	import { createCommitStore } from '$lib/vbranches/contexts';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import type { LocalFile, RemoteFile } from '$lib/vbranches/types';

	export let files: LocalFile[] | RemoteFile[];
	export let isUnapplied: boolean;
	export let showCheckboxes = false;

	export let allowMultiple = false;
	export let readonly = false;

	createCommitStore(undefined);
	const fileIdSelection = getContext(FileIdSelection);

	function unselectAllFiles() {
		fileIdSelection.clear();
	}
</script>

<div
	class="branch-files"
	role="listbox"
	tabindex="-1"
	on:keydown={(e) => {
		if (e.key === 'Escape') {
			unselectAllFiles();
		}
	}}
	on:click={unselectAllFiles}
>
	{#if files.length > 0}
		<BranchFilesList {allowMultiple} {readonly} {files} {showCheckboxes} {isUnapplied} />
	{/if}
</div>

<style lang="postcss">
	.branch-files {
		flex: 1;
	}
</style>
