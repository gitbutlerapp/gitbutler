<script lang="ts">
	import ArrowButton from '$home/components/ArrowButton.svelte';
	import SectionHeader from '$home/components/SectionHeader.svelte';
	import ReleaseCard from '$lib/components/ReleaseCard.svelte';
	import type { Release } from '$lib/types/releases';

	interface Props {
		releases: Release[];
	}

	const { releases }: Props = $props();

	let visibleCount = $state(2);

	function showMore() {
		visibleCount = Math.min(visibleCount + 2, 10);
	}

	function goToFullChangelog() {
		window.open('https://github.com/gitbutlerapp/gitbutler/releases', '_blank');
	}
</script>

<section class="changelog-section">
	<SectionHeader>
		Changelog

		{#snippet buttons()}
			<ArrowButton
				label="All updates"
				onclick={() => window.open('https://github.com/gitbutlerapp/gitbutler/releases', '_blank')}
			/>
		{/snippet}
	</SectionHeader>

	<div class="changelog">
		{#if releases && releases.length > 0}
			<div class="release-list">
				{#each releases.slice(0, visibleCount) as release}
					<ReleaseCard {release} />
				{/each}
			</div>

			{#if visibleCount < 10 && releases.length > visibleCount}
				<div class="show-more-container">
					<button type="button" class="show-more-button" onclick={showMore}> Show more </button>
				</div>
			{:else if visibleCount >= 10 || releases.length <= visibleCount}
				<div class="show-more-container">
					<button type="button" class="show-more-button full-changelog" onclick={goToFullChangelog}>
						See complete changelog
					</button>
				</div>
			{/if}
		{:else}
			<div class="loading">Loading releases...</div>
		{/if}
	</div>
</section>

<style>
	.changelog-section {
		display: grid;
		grid-template-columns: subgrid;
		grid-column: full-start / full-end;
	}

	.changelog {
		display: flex;
		grid-column: narrow-start / narrow-end;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-xl);
		font-family: var(--fontfamily-mono);
	}

	.loading {
		grid-column: narrow-start / narrow-end;
		padding: 2rem;
		color: var(--color-text-secondary);
		text-align: center;
	}

	.show-more-container {
		display: flex;
		z-index: 1;
		position: relative;
		justify-content: center;
		padding: 0px 20px 10px;

		&::after {
			z-index: 0;
			position: absolute;
			bottom: 0;
			left: 0;
			width: 100%;
			height: 60px;
			background: linear-gradient(to top, var(--clr-bg-3) 40%, transparent);
			content: '';
			pointer-events: none;
		}
	}

	.show-more-button {
		z-index: 1;
		position: relative;
		padding: 10px 6px;
		font-size: 13px;
		text-decoration: dotted underline;
		text-underline-offset: 2px;
	}
</style>
