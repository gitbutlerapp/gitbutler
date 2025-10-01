<script lang="ts">
	import osIcons from '$lib/data/os-icons.json';
	import { marked } from '@gitbutler/ui/utils/marked';
	import type { Release } from '$lib/types/releases';

	interface Props {
		release: Release;
		showSeparator?: boolean;
		showDownloadLinks?: boolean;
	}

	const { release, showSeparator = true, showDownloadLinks = false }: Props = $props();

	// State for toggling download links visibility
	let downloadLinksVisible = $state(false);

	// Group builds by OS for easier display
	const groupedBuilds = $derived.by(() => {
		const builds =
			release.builds?.reduce(
				(acc, build) => {
					if (!acc[build.os]) acc[build.os] = [];
					acc[build.os].push(build);
					return acc;
				},
				{} as Record<string, typeof release.builds>
			) || {};

		// Sort OS entries: macOS (darwin) first, then Windows, then Linux
		const osOrder = ['darwin', 'windows', 'linux'];
		return Object.fromEntries(
			Object.entries(builds).sort(([a], [b]) => {
				const aIndex = osOrder.indexOf(a);
				const bIndex = osOrder.indexOf(b);
				const aOrder = aIndex === -1 ? osOrder.length : aIndex;
				const bOrder = bIndex === -1 ? osOrder.length : bIndex;
				return aOrder - bOrder;
			})
		);
	});

	function getBuildDisplayName(build: any): string {
		if (build.os === 'darwin') {
			return build.arch === 'aarch64' ? 'Apple Silicon' : 'Intel';
		}
		if (build.os === 'windows') {
			return 'MSI';
		}
		if (build.os === 'linux') {
			if (build.platform.includes('appimage')) return 'AppImage';
			if (build.platform.includes('deb')) return 'Deb';
			if (build.platform.includes('rpm')) return 'RPM';
		}
		return build.platform;
	}
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
				{@html marked(release.notes)}
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
				<div class="download-links">
					{#each Object.entries(groupedBuilds) as [os, builds]}
						<div class="download-category">
							<svg
								class="download-icon"
								viewBox="0 0 22 22"
								fill="none"
								xmlns="http://www.w3.org/2000/svg"
							>
								<path
									d={osIcons[os === 'darwin' ? 'macos' : (os as keyof typeof osIcons)]}
									fill="currentColor"
								/>
							</svg>
							<div class="download-options">
								{#each builds as build}
									<a href={build.url} class="download-link">
										{getBuildDisplayName(build)}
									</a>
								{/each}
							</div>
						</div>
					{/each}
				</div>
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
		font-family: var(--fontfamily-accent);
	}

	.release-date {
		font-size: 12px;
		white-space: nowrap;
		opacity: 0.6;
	}

	.release-notes-content {
		font-size: 13px;
		font-family: var(--fontfamily-mono);
	}

	.download-links-toggle {
		width: fit-content;
		color: var(--clr-text-2);
		font-size: 13px;
		font-family: var(--fontfamily-mono);

		text-underline-offset: 3px;
		cursor: pointer;
		transition: color 0.1s ease;

		span:first-child {
			text-decoration: dotted underline;
		}

		&:hover {
			color: var(--clr-text-1);
		}
	}

	.download-links {
		display: flex;
		flex-wrap: wrap;
		padding: 32px;
		gap: 24px;
		background-image: radial-gradient(var(--clr-border-2) 1px, transparent 1px);
		background-size: 7px 7px;
	}

	.download-category {
		display: flex;
		flex-direction: column;
		min-width: 150px;
		gap: 12px;
	}

	.download-icon {
		width: 22px;
		height: 22px;
	}

	.download-options {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.download-link {
		font-size: 12px;
		font-family: var(--fontfamily-mono);
		text-decoration: underline;
		text-underline-offset: 2px;
		transition: all 0.1s ease;

		&:hover {
			text-decoration: underline wavy;
			text-decoration-color: var(--clr-theme-pop-element);
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
