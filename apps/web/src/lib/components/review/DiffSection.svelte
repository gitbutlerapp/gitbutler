<script lang="ts">
	import { splitDiffIntoHunks } from '$lib/diffParsing';
	import HunkDiff from '@gitbutler/ui/HunkDiff.svelte';
	import type { DiffSection } from '@gitbutler/shared/branches/types';

	interface Props {
		section: DiffSection;
	}
	const { section }: Props = $props();

	const hunks = $derived(section.diffPatch ? splitDiffIntoHunks(section.diffPatch) : []);
</script>

<div class="diff-section">
	<p class="file-name">{section.newPath}</p>
	{#each hunks as hunkStr}
		<HunkDiff filePath={section.newPath || 'unknown'} {hunkStr}></HunkDiff>
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
	}

	.file-name {
		color: var(--text-1, #1a1614);

		/* base-body/12 */
		font-family: var(--fontfamily-default, Inter);
		font-size: 12px;
		font-style: normal;
		font-weight: var(--weight-regular, 400);
		line-height: 160%; /* 19.2px */
	}
</style>
