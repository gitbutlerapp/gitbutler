<script lang="ts">
	import { splitDiffIntoHunks } from '$lib/diffParsing';
	import HunkDiff, { type LineClickParams } from '@gitbutler/ui/HunkDiff.svelte';
	import FileIcon from '@gitbutler/ui/file/FileIcon.svelte';
	import type { DiffSection } from '@gitbutler/shared/branches/types';
	import type { ContentSection, LineSelector } from '@gitbutler/ui/utils/diffParsing';

	interface Props {
		section: DiffSection;
		selectedLines: LineSelector[];
		clearLineSelection: () => void;
		toggleDiffLine: (
			fileName: string,
			hunkIndex: number,
			diffSha: string,
			params: LineClickParams
		) => void;
		onCopySelection: (contentSections: ContentSection[]) => void;
		onQuoteSelection: () => void;
	}
	const {
		section,
		toggleDiffLine,
		selectedLines,
		onCopySelection,
		onQuoteSelection,
		clearLineSelection
	}: Props = $props();

	const hunks = $derived(section.diffPatch ? splitDiffIntoHunks(section.diffPatch) : []);
	const filePath = $derived(section.newPath || 'unknown');

	function handleLineClick(index: number, params: LineClickParams) {
		toggleDiffLine(section.newPath || 'unknown', index, section.diffSha, params);
	}
</script>

<div class="diff-section">
	<div class="diff-section__header">
		<FileIcon fileName={filePath} size={16} />
		<p title={filePath} class="text-12 text-body file-name">{filePath}</p>
	</div>
	{#each hunks as hunkStr, idx}
		<HunkDiff
			filePath={section.newPath || 'unknown'}
			{hunkStr}
			diffLigatures={false}
			{selectedLines}
			onLineClick={(p) => handleLineClick(idx, p)}
			{onCopySelection}
			{onQuoteSelection}
			{clearLineSelection}
		/>
	{/each}
</div>

<style lang="postcss">
	.diff-section {
		display: flex;
		padding: 14px;
		flex-direction: column;
		align-items: flex-start;
		gap: 14px;
		align-self: stretch;

		&:not(:last-child) {
			border-bottom: 1px solid var(--clr-border-2);
		}
	}

	.diff-section__header {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.file-name {
		color: var(--clr-text-1);
	}
</style>
