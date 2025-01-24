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
		<p class="statistic-files">{patch.statistics.fileCount} files changed</p>
		<p class="statistic-added">{patch.statistics.lines} additions</p>
		<p class="statistic-deleted">{patch.statistics.deletions} deletions</p>
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

		border-radius: var(--ml, 10px);
		border: 1px solid var(--border-2, #d4d0ce);
		background: var(--bg-1, #fff);
	}

	.review-sections-statistics {
		height: 48px;
		display: flex;
		gap: 8px;
		padding: 17px 16px;
		align-items: center;
		align-self: stretch;
		border-bottom: 1px solid var(--border-2, #d4d0ce);
	}

	.statistic-files {
		color: var(--text-1, #1a1614);

		/* base/12-bold */
		font-family: var(--fontfamily-default, Inter);
		font-size: 12px;
		font-style: normal;
		font-weight: var(--weight-bold, 600);
		line-height: 120%; /* 14.4px */
	}

	.statistic-added {
		color: var(--succ-30, #287b55);

		/* base/12 */
		font-family: var(--fontfamily-default, Inter);
		font-size: 12px;
		font-style: normal;
		font-weight: var(--weight-regular, 400);
		line-height: 120%; /* 14.4px */
	}

	.statistic-deleted {
		color: var(--err-30, #95323c);

		/* base/12 */
		font-family: var(--fontfamily-default, Inter);
		font-size: 12px;
		font-style: normal;
		font-weight: var(--weight-regular, 400);
		line-height: 120%; /* 14.4px */
	}
</style>
