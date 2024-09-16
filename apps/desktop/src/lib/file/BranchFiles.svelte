<script lang="ts">
	import BranchFilesList from './BranchFilesList.svelte';
	import { getContext } from '$lib/utils/context';
	import { createCommitStore } from '$lib/vbranches/contexts';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import type { LocalFile, RemoteFile } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let files: LocalFile[] | RemoteFile[];
	export let isUnapplied: boolean;
	export let showCheckboxes = false;
	export let commitDialogExpanded: Writable<boolean>;
	export let focusCommitDialog: () => void;

	export let allowMultiple = false;
	export let readonly = false;

	createCommitStore(undefined);
	const fileIdSelection = getContext(FileIdSelection);
</script>

<div class="branch-files" role="presentation" on:click={() => fileIdSelection.clear()}>
	{#if files.length > 0}
		<BranchFilesList
			{allowMultiple}
			{readonly}
			{files}
			{showCheckboxes}
			{isUnapplied}
			{commitDialogExpanded}
			{focusCommitDialog}
		/>
	{/if}
</div>

<style lang="postcss">
	.branch-files {
		flex: 1;
	}

	.branch-files:focus {
		outline: none;
	}
</style>
