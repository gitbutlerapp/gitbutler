<script lang="ts">
	import BranchFilesList from './BranchFilesList.svelte';
	import { createCommitStore } from '$lib/vbranches/contexts';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import { getContext } from '@gitbutler/shared/context';
	import type { LocalFile, RemoteFile } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	interface Props {
		files: LocalFile[] | RemoteFile[];
		isUnapplied: boolean;
		showCheckboxes?: boolean;
		commitDialogExpanded: Writable<boolean>;
		focusCommitDialog: () => void;
		allowMultiple?: boolean;
		readonly?: boolean;
	}

	let {
		files,
		isUnapplied,
		showCheckboxes = false,
		commitDialogExpanded,
		focusCommitDialog,
		allowMultiple = false,
		readonly = false
	}: Props = $props();

	createCommitStore(undefined);
	const fileIdSelection = getContext(FileIdSelection);
</script>

<div class="branch-files" role="presentation" onclick={() => fileIdSelection.clear()}>
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
