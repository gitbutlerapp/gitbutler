<script lang="ts">
	import { splitDiffIntoHunks } from '$lib/diffParsing';
	import HunkDiff from '@gitbutler/ui/HunkDiff.svelte';
	import FileIcon from '@gitbutler/ui/file/FileIcon.svelte';
	import type { DiffSection } from '@gitbutler/shared/branches/types';

	interface Props {
		section: DiffSection;
	}
	const { section }: Props = $props();

	const hunks = $derived(section.diffPatch ? splitDiffIntoHunks(section.diffPatch) : []);
	const filePath = $derived(section.newPath || 'unknown');
</script>

<div class="diff-section">
	<div class="diff-section__header">
		<FileIcon fileName={filePath} size={16} />
		<p title={filePath} class="text-12 text-body file-name">{filePath}</p>
	</div>
	{#each hunks as hunkStr}
		<HunkDiff filePath={section.newPath || 'unknown'} {hunkStr} diffLigatures={false}></HunkDiff>
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
