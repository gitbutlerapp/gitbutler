<script lang="ts">
	import DiffSection from '$lib/components/review/DiffSection.svelte';
	import { type Section } from '@gitbutler/shared/patches/types';
	import type { LineClickParams } from '@gitbutler/ui/HunkDiff.svelte';
	import type { ContentSection, LineSelector } from '@gitbutler/ui/utils/diffParsing';

	interface Props {
		isLoggedIn: boolean;
		section: Section;
		selectedSha: string | undefined;
		selectedLines: LineSelector[];
		commitPageHeaderHeight: number;
		clearLineSelection: (fileName: string) => void;
		toggleDiffLine: (fileName: string, diffSha: string, params: LineClickParams) => void;
		onCopySelection: (contentSections: ContentSection[]) => void;
		onQuoteSelection: () => void;
	}

	const {
		isLoggedIn,
		section,
		toggleDiffLine,
		selectedSha,
		selectedLines,
		commitPageHeaderHeight,
		onCopySelection,
		onQuoteSelection,
		clearLineSelection
	}: Props = $props();
</script>

{#if section.sectionType === 'diff'}
	<DiffSection
		{isLoggedIn}
		{section}
		{toggleDiffLine}
		{selectedSha}
		{selectedLines}
		{commitPageHeaderHeight}
		{onCopySelection}
		{onQuoteSelection}
		{clearLineSelection}
	/>
{/if}
