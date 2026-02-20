<script lang="ts">
	import ReleaseDownloadLinks from '$lib/components/marketing/ReleaseDownloadLinks.svelte';
	import Markdown from 'svelte-exmarkdown';
	import type { Release } from '$lib/types/releases';

	interface Props {
		release: Release;
		showSeparator?: boolean;
		showDownloadLinks?: boolean;
	}

	const { release, showSeparator = true, showDownloadLinks = false }: Props = $props();

	// State for toggling download links visibility
	let downloadLinksVisible = $state(false);
</script>

<div class="release" class:no-separator={!showSeparator}>
	<div class="release-header">
		<h3 class="release-version">{release.version}</h3>
		<span class="release-date">
			{new Date(release.released_at).toLocaleDateString('en-GB', {
				day: 'numeric',
				month: 'short',
				year: 'numeric'
			})}
		</span>
	</div>

	<div class="stack-v gap-16">
		{#if release.notes}
			<div class="release-notes-content">
				<Markdown md={release.notes} />
			</div>
		{/if}

		{#if showDownloadLinks && release.builds && release.builds.length > 0}
			{#if !downloadLinksVisible}
				<button
					type="button"
					class="download-links-toggle"
					onclick={() => (downloadLinksVisible = true)}
				>
					<span>Show download options</span>
					<span>⭳</span>
				</button>
			{/if}

			{#if downloadLinksVisible}
				<ReleaseDownloadLinks builds={release.builds} />
			{/if}
		{/if}
	</div>
</div>

<style>
	.release {
		display: flex;
		position: relative;
		padding: 20px;
		padding-bottom: 36px;
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
				var(--clr-text-2),
				var(--clr-text-2) 2px,
				transparent 2px,
				transparent 6px
			);

			content: '';
			pointer-events: none;
		}

		&.no-separator:after,
		&:last-child:after {
			display: none;
		}
	}

	.release-header {
		display: flex;
		flex-direction: column;
	}

	.release-version {
		font-size: 42px;
		font-family: var(--font-accent);
	}

	.release-date {
		font-size: 12px;
		white-space: nowrap;
		opacity: 0.6;
	}

	.release-notes-content {
		font-size: 13px;
		font-family: var(--font-mono);
	}

	.download-links-toggle {
		width: fit-content;
		color: var(--clr-text-2);
		font-size: 13px;
		font-family: var(--font-mono);

		text-underline-offset: 3px;
		transition: color 0.1s ease;

		span:first-child {
			text-decoration: dotted underline;
		}

		&:hover {
			color: var(--clr-text-1);
		}
	}

	@media (--mobile-viewport) {
		.release {
			flex-direction: column;
			padding: 16px;
			padding-bottom: 28px;
			gap: 24px;
		}
	}
</style>
