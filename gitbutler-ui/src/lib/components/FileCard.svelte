<script lang="ts">
	import FileCardHeader from './FileCardHeader.svelte';
	import FileDiff from './FileDiff.svelte';
	import ScrollableContainer from '$lib/components/ScrollableContainer.svelte';
	import { getContextByClass } from '$lib/utils/context';
	import { ContentSection, HunkSection, parseFileSections } from '$lib/utils/fileSections';
	import { BranchController } from '$lib/vbranches/branchController';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { AnyFile, Commit } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let branchId: string;
	export let file: AnyFile;
	export let conflicted: boolean;
	export let projectPath: string | undefined;
	export let isUnapplied: boolean;
	export let selectable = false;
	export let readonly = false;
	export let selectedOwnership: Writable<Ownership>;
	export let branchCommits: Commit[] = [];

	const branchController = getContextByClass(BranchController);

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

<div id={`file-${file.id}`} class="file-card card">
	<FileCardHeader {file} {isFileLocked} on:close />
	{#if conflicted}
		<div class="mb-2 bg-red-500 px-2 py-0 font-bold text-white">
			<button
				class="font-bold text-white"
				on:click={() => branchController.markResolved(file.path)}
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
			{projectPath}
			{isFileLocked}
			{isUnapplied}
			{selectable}
			{branchId}
			{selectedOwnership}
			{branchCommits}
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

		/* background-color: red; */
		/* background-color: var(--clr-theme-container-outline-light); */

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
		background: var(--clr-theme-container-light);
		overflow: hidden;
		display: flex;
		flex-direction: column;
		max-height: 100%;
		flex-grow: 1;
	}

	@keyframes wiggle {
		0% {
			transform: rotate(0deg);
		}
		40% {
			transform: rotate(0deg);
		}
		60% {
			transform: rotate(2deg);
		}
		80% {
			transform: rotate(-2deg);
		}
		100% {
			transform: rotate(0deg);
		}
	}
	:global(.wiggle) {
		animation: wiggle 0.5s infinite;
	}
</style>
