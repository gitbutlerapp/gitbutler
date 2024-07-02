<script lang="ts">
	import FileCardHeader from './FileCardHeader.svelte';
	import FileDiff from './FileDiff.svelte';
	import ScrollableContainer from '$lib/shared/ScrollableContainer.svelte';
	import { getContext } from '$lib/utils/context';
	import { ContentSection, HunkSection, parseFileSections } from '$lib/utils/fileSections';
	import { BranchController } from '$lib/vbranches/branchController';
	import type { AnyFile } from '$lib/vbranches/types';

	export let file: AnyFile;
	export let conflicted: boolean;
	export let isUnapplied: boolean;
	export let selectable = false;
	export let readonly = false;
	export let isCard = true;

	const branchController = getContext(BranchController);

	let sections: (HunkSection | ContentSection)[] = [];

	function parseFile(file: AnyFile) {
		// When we toggle expansion status on sections we need to assign
		// `sections = sections` to redraw, and why we do not use a reactive
		// variable.
		if (!file.binary && !file.large) sections = parseFileSections(file);
	}
	$: parseFile(file);

	$: isFileLocked = sections
		.filter((section): section is HunkSection => section instanceof HunkSection)
		.some((section) => section.hunk.locked);
</script>

<div id={`file-${file.id}`} class="file-card" class:card={isCard}>
	<FileCardHeader {file} {isFileLocked} on:close />
	{#if conflicted}
		<div class="mb-2 bg-red-500 px-2 py-0 font-bold text-white">
			<button
				class="font-bold text-white"
				on:click={async () => await branchController.markResolved(file.path)}
			>
				Mark resolved
			</button>
		</div>
	{/if}

	<ScrollableContainer wide>
		<FileDiff
			filePath={file.path}
			isLarge={file.large}
			isBinary={file.binary}
			{readonly}
			{sections}
			{isFileLocked}
			{isUnapplied}
			{selectable}
		/>
	</ScrollableContainer>
</div>

<div class="divider-line"></div>

<style lang="postcss">
	.divider-line {
		position: absolute;
		top: 0;
		right: 0;
		width: 1px;
		height: 100%;

		&:after {
			pointer-events: none;
			content: '';
			position: absolute;
			top: 0;
			right: 50%;
			transform: translateX(50%);
			width: 1px;
			height: 100%;
		}
	}

	.file-card {
		background: var(--clr-bg-1);
		overflow: hidden;
		display: flex;
		flex-direction: column;
		max-height: 100%;
		flex-grow: 1;
	}
</style>
