<script lang="ts">
	import { splitDiffIntoHunks } from '$lib/diffParsing';
	import HunkDiff, { type LineClickParams } from '@gitbutler/ui/HunkDiff.svelte';
	import FileIcon from '@gitbutler/ui/file/FileIcon.svelte';
	import type { DiffSection } from '@gitbutler/shared/branches/types';
	import type { ContentSection, LineSelector } from '@gitbutler/ui/utils/diffParsing';

	interface Props {
		section: DiffSection;
		selectedSha: string | undefined;
		selectedLines: LineSelector[];
		clearLineSelection: (fileName: string) => void;
		toggleDiffLine: (fileName: string, diffSha: string, params: LineClickParams) => void;
		onCopySelection: (contentSections: ContentSection[]) => void;
		onQuoteSelection: () => void;
	}
	const {
		section,
		toggleDiffLine,
		selectedSha,
		selectedLines: lines,
		onCopySelection,
		onQuoteSelection,
		clearLineSelection
	}: Props = $props();

	const hunks = $derived(section.diffPatch ? splitDiffIntoHunks(section.diffPatch) : []);
	const filePath = $derived(section.newPath || 'unknown');

	function handleLineClick(params: LineClickParams) {
		toggleDiffLine(section.newPath || 'unknown', section.diffSha, params);
	}

	const selectedLines = $derived(selectedSha === section.diffSha ? lines : []);
</script>

<div class="diff-section">
	<div class="diff-section__header">
		<FileIcon fileName={filePath} size={16} />
		<p title={filePath} class="text-12 text-body file-name">{filePath}</p>
	</div>
	{#each hunks as hunkStr}
		<HunkDiff
			filePath={section.newPath || 'unknown'}
			{hunkStr}
			diffLigatures={false}
			{selectedLines}
			onLineClick={handleLineClick}
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
