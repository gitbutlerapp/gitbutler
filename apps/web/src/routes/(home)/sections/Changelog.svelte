<script lang="ts">
	import ArrowButton from '$home/components/ArrowButton.svelte';
	import SectionHeader from '$home/components/SectionHeader.svelte';
	import { marked } from '@gitbutler/ui/utils/marked';
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
					<div class="release">
						<div class="release-header">
							<h3 class="release-version">{release.version}</h3>
							<span class="release-date">
								{new Date(release.released_at).toLocaleDateString()}
							</span>
						</div>
						{#if release.notes}
							<div class="release-notes">
								{@html marked(release.notes)}
							</div>
						{/if}
					</div>
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

	.release {
		display: flex;
		position: relative;
		padding: 20px;
		gap: 32px;

		&:after {
			z-index: 0;
			position: absolute;
			right: 20px;
			bottom: 0;
			left: 20px;
			height: 1px;
			background: repeating-linear-gradient(
				to right,
				var(--clr-text-3),
				var(--clr-text-3) 2px,
				transparent 2px,
				transparent 6px
			);

			content: '';
			pointer-events: none;
		}

		&:last-child:after {
			display: none;
		}
	}

	.release-header {
		display: flex;
		flex-direction: column;
	}

	.release-version {
		font-size: 40px;
		font-family: var(--fontfamily-accent);
	}

	.release-date {
		font-size: 13px;
		opacity: 0.6;
	}

	.release-notes {
		font-size: 13px;
		line-height: 1.6;
	}

	.release-notes :global(h1),
	.release-notes :global(h2),
	.release-notes :global(h3),
	.release-notes :global(h4),
	.release-notes :global(h5),
	.release-notes :global(h6) {
		margin-top: 1.5rem;
		margin-bottom: 0.5rem;
		font-weight: 600;
	}

	/* remove top margin for first heading */
	.release-notes :global(h1:first-child),
	.release-notes :global(h2:first-child),
	.release-notes :global(h3:first-child),
	.release-notes :global(h4:first-child),
	.release-notes :global(h5:first-child),
	.release-notes :global(h6:first-child) {
		margin-top: 0.6rem;
	}

	.release-notes :global(p) {
		margin: 0.12px 0;
	}

	.release-notes :global(ul),
	.release-notes :global(ol) {
		margin: 12px 0;
		padding-left: 1rem;
	}

	.release-notes :global(li) {
		margin: 8px 0;
		padding-left: 4px;
		/* style */
		list-style-type: disc;
	}

	.release-notes :global(code) {
		padding: 0.125rem 0.25rem;
		border-radius: 0.25rem;
		background-color: var(--color-bg-secondary);
		font-size: 0.875rem;
		font-family: var(--font-mono);
	}

	.release-notes :global(pre) {
		margin: 1rem 0;
		padding: 1rem;
		overflow-x: auto;
		border-radius: 0.5rem;
		background-color: var(--color-bg-secondary);
	}

	.release-notes :global(blockquote) {
		margin: 1rem 0;
		padding-left: 1rem;
		border-left: 3px solid var(--color-primary);
		font-style: italic;
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

	@media (--mobile-viewport) {
		.release {
			flex-direction: column;
			padding: 16px;
			gap: 24px;
		}
	}
</style>
