<script lang="ts">
	import SectionComponent from './Section.svelte';
	import type { Patch, Section } from '@gitbutler/shared/branches/types';

	interface Props {
		patch: Patch;
		patchSections: Section[] | undefined;
	}

	const { patch, patchSections }: Props = $props();
</script>

<div class="review-sections-card">
	<div class="review-sections-statistics">
		<p class="text-12 text-bold statistic-files">{patch.statistics.fileCount} files changed</p>
		<p class="text-12 statistic-added">
			{patch.statistics.lines - patch.statistics.deletions} additions
		</p>
		<p class="text-12 statistic-deleted">{patch.statistics.deletions} deletions</p>
	</div>
	{#if patchSections !== undefined}
		{#each patchSections as section}
			<SectionComponent {section} />
		{/each}
	{/if}
</div>

<style>
	.review-sections-card {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		align-self: stretch;

		border-radius: var(--radius-ml);
		border: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
	}

	.review-sections-statistics {
		height: 48px;
		display: flex;
		gap: 8px;
		padding: 17px 16px;
		align-items: center;
		align-self: stretch;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.statistic-files {
		color: var(--clr-text-1);
	}

	.statistic-added {
		color: var(--clr-scale-succ-30);
	}

	.statistic-deleted {
		color: var(--clr-scale-err-30);
	}
</style>
