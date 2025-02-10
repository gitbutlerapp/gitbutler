<script lang="ts">
	import DiffSection from './DiffSection.svelte';
	import { type Section } from '@gitbutler/shared/branches/types';
	import type { LineClickParams } from '@gitbutler/ui/HunkDiff.svelte';
	import type { LineSelector } from '@gitbutler/ui/utils/diffParsing';

	interface Props {
		section: Section;
		selectedLines: LineSelector[];
		toggleDiffLine: (
			fileName: string,
			hunkIndex: number,
			diffSha: string,
			params: LineClickParams
		) => void;
	}

	const { section, toggleDiffLine, selectedLines }: Props = $props();
</script>

{#if section.sectionType === 'diff'}
	<DiffSection {section} {toggleDiffLine} {selectedLines} />
{/if}
