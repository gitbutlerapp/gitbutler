<script lang="ts">
	import DiffSection from './DiffSection.svelte';
	import { type Section } from '@gitbutler/shared/branches/types';
	import type { LineClickParams } from '@gitbutler/ui/HunkDiff.svelte';
	import type { ContentSection, LineSelector } from '@gitbutler/ui/utils/diffParsing';

	interface Props {
		section: Section;
		selectedSha: string | undefined;
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
		selectedSha,
		selectedLines,
		onCopySelection,
		onQuoteSelection,
		clearLineSelection
	}: Props = $props();
</script>

{#if section.sectionType === 'diff'}
	<DiffSection
		{section}
		{toggleDiffLine}
		{selectedSha}
		{selectedLines}
		{onCopySelection}
		{onQuoteSelection}
		{clearLineSelection}
	/>
{/if}
