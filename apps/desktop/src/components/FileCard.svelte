<script lang="ts">
	import FileCardHeader from './FileCardHeader.svelte';
	import FileDiff from './FileDiff.svelte';
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import { BranchController } from '$lib/branches/branchController';
	import { ContentSection, HunkSection, parseFileSections } from '$lib/utils/fileSections';
	import { getContext } from '@gitbutler/shared/context';
	import type { AnyFile } from '$lib/files/file';

	interface Props {
		file: AnyFile;
		conflicted: boolean;
		isUnapplied: boolean;
		selectable?: boolean;
		readonly?: boolean;
		isCard?: boolean;
		commitId?: string;
		onClose?: () => void;
	}

	const {
		file,
		conflicted,
		isUnapplied,
		commitId,
		selectable = false,
		readonly = false,
		isCard = true,
		onClose
	}: Props = $props();

	const branchController = getContext(BranchController);

	let sections: (HunkSection | ContentSection)[] = $state([]);

	function parseFile(file: AnyFile) {
		// When we toggle expansion status on sections we need to assign
		// `sections = sections` to redraw, and why we do not use a reactive
		// variable.
		if (!file.binary && !file.large) sections = parseFileSections(file);
	}
	$effect(() => parseFile(file));

	const isFileLocked = $derived(
		sections
			.filter((section): section is HunkSection => section instanceof HunkSection)
			.some((section) => section.hunk.locked)
	);
</script>

<div id={`file-${file.id}`} class="file-card" class:card={isCard}>
	<FileCardHeader {file} {isFileLocked} {onClose} />
	{#if conflicted}
		<div class="file-card__resolved-btn">
			<button
				type="button"
				class="font-bold text-white"
				onclick={async () => await branchController.markResolved(file.path)}
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
			{commitId}
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

	.file-card__resolved-btn {
		margin-bottom: 0.25rem;
		background-color: var(--clr-theme-err-soft);
		padding: 0.5rem;
	}
</style>
