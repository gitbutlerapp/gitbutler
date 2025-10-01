<script lang="ts">
	import MarketingFooter from '$lib/components/MarketingFooter.svelte';
	import MarketingHeader from '$lib/components/MarketingHeader.svelte';
	import osIcons from '$lib/data/os-icons.json';
	import type { Build, Release } from '$lib/types/releases';

	interface Props {
		data: {
			nightlies: Release[];
			latestNightly: Release | null;
			latestNightlyBuilds: { [key: string]: Build };
		};
	}

	const { data }: Props = $props();

	const { latestNightly, latestNightlyBuilds } = data;

	let expandedRelease: string | null = $state(null);

	function toggleRelease(version: string) {
		expandedRelease = expandedRelease === version ? null : version;
	}
</script>

<svelte:head>
	<title>GitButler | Nightly Builds</title>
</svelte:head>

<section class="latest-nightly-wrapper">
	<MarketingHeader />

	{#if latestNightly}
		<div class="nightly-hero">
			<div class="nightly-hero__header">
				<img class="nightly-hero__header-icon" src="/images/app-icon-nightly.svg" alt="" />
				<div class="nightly-hero__header-labels">
					<h1>{latestNightly.version}</h1>
					<div class="nightly-hero__header-details">
						<span>Latest release</span>
						<span> • </span>
						<span
							>{new Date(latestNightly.released_at).toLocaleDateString('en-GB', {
								day: 'numeric',
								month: 'long',
								year: 'numeric'
							})} at {new Date(latestNightly.released_at).toLocaleTimeString('en-GB', {
								hour: '2-digit',
								minute: '2-digit',
								hour12: false
							})}
						</span>
					</div>
					<p class="nightly-hero__description">
						Experience GitButler's newest features before anyone else. Nightly builds are
						automatically generated from the latest development code and may contain experimental
						features and bugs.
					</p>
				</div>
			</div>

			<div class="latest-nightly">
				<h2 class="latest-nightly__header">
					<span> Download Nightly </span>
					<svg
						class="arrow-down"
						viewBox="0 0 34 33"
						fill="none"
						xmlns="http://www.w3.org/2000/svg"
					>
						<path d="M23.1667 27L17.3333 20H23.1667H29L23.1667 27Z" fill="currentColor" />
						<path
							d="M4 9.5H23.1667V27M23.1667 27L17.3333 20H29L23.1667 27Z"
							stroke="currentColor"
							stroke-width="2"
						/>
					</svg>
				</h2>

				<div class="download-links">
					<div class="download-category">
						<svg
							class="download-icon"
							viewBox="0 0 22 22"
							fill="none"
							xmlns="http://www.w3.org/2000/svg"
						>
							<path d={osIcons.macos} fill="currentColor" />
						</svg>
						<div class="download-options">
							{#if latestNightlyBuilds.darwin_x86_64}
								<a href={(latestNightlyBuilds.darwin_x86_64 as Build).url} class="download-link">
									macOS Intel
								</a>
							{/if}
							{#if latestNightlyBuilds.darwin_aarch64}
								<a href={(latestNightlyBuilds.darwin_aarch64 as Build).url} class="download-link">
									macOS Apple Silicon
								</a>
							{/if}
						</div>
					</div>

					<div class="download-category">
						<svg
							class="download-icon"
							viewBox="0 0 22 22"
							fill="none"
							xmlns="http://www.w3.org/2000/svg"
						>
							<path d={osIcons.windows} fill="currentColor" />
						</svg>
						<div class="download-options">
							{#if latestNightlyBuilds.windows_x86_64}
								<a href={(latestNightlyBuilds.windows_x86_64 as Build).url} class="download-link">
									Windows (MSI)
								</a>
							{/if}
						</div>
					</div>

					<div class="download-category">
						<svg
							class="download-icon"
							viewBox="0 0 22 22"
							fill="none"
							xmlns="http://www.w3.org/2000/svg"
						>
							<path d={osIcons.linux} fill="currentColor" />
						</svg>
						<div class="download-options">
							{#if latestNightlyBuilds.linux_appimage}
								<a href={(latestNightlyBuilds.linux_appimage as Build).url} class="download-link"
									>Linux Intel (AppImage)</a
								>
							{/if}
							{#if latestNightlyBuilds.linux_deb}
								<a href={(latestNightlyBuilds.linux_deb as Build).url} class="download-link">
									Linux Intel (Deb)
								</a>
							{/if}
							{#if latestNightlyBuilds.linux_rpm}
								<a href={(latestNightlyBuilds.linux_rpm as Build).url} class="download-link">
									Linux Intel (RPM)
								</a>
							{/if}
						</div>
					</div>
				</div>
			</div>

			<p class="nightly-warning">
				⚠️ Nightly builds are experimental and may be unstable. Use at your own risk.
				<br />
				For production use, please use the
				<a href="/downloads">stable release</a>.
			</p>

			<div class="background__noisy noisy-1"></div>
			<div class="background__noisy noisy-2"></div>
		</div>
	{:else}
		<div class="no-nightly">
			<p class="text-16 clr-text-2">No nightly builds are currently available.</p>
		</div>
	{/if}
</section>

{#if data.nightlies.length > 0}
	<section class="releases">
		<h3>
			Other <i>nightly</i> builds:
		</h3>

		{#each data.nightlies as release (release.version)}
			<div class="release-row">
				<button
					type="button"
					class="release-row__button"
					class:expanded={expandedRelease === release.version}
					onclick={() => toggleRelease(release.version)}
				>
					<span class="release-row__version">{release.version}</span>
					<span class="release-row__date">
						{new Date(release.released_at).toLocaleDateString('en-GB', {
							day: 'numeric',
							month: 'short',
							year: 'numeric'
						})}
					</span>
				</button>

				{#if expandedRelease === release.version}
					<div class="release-row__links">
						{#if release.builds && release.builds.length > 0}
							{#each release.builds as build}
								<a href={build.url} class="download-link">
									{#if build.os === 'darwin'}
										{#if build.platform.includes('aarch64')}
											macOS Apple Silicon
										{:else if build.platform.includes('x86_64')}
											macOS Intel
										{:else}
											macOS {build.platform}
										{/if}
									{:else if build.os === 'windows'}
										MSI
									{:else if build.os === 'linux'}
										{#if build.platform.includes('appimage')}
											Linux Intel (AppImage)
										{:else if build.platform.includes('deb')}
											Linux Intel (Deb)
										{:else if build.platform.includes('rpm')}
											Linux Intel (RPM)
										{:else}
											Linux {build.platform}
										{/if}
									{:else}
										{build.os} {build.platform}
									{/if}
								</a>
							{/each}
						{/if}
					</div>
				{/if}
			</div>
		{/each}
	</section>
{/if}

<MarketingFooter showDownloadLinks={false} />

<style>
	.latest-nightly-wrapper {
		display: grid;
		grid-template-columns: subgrid;
		grid-column: full-start / full-end;
		gap: 30px;
	}

	.nightly-hero {
		display: flex;
		position: relative;
		grid-column: narrow-start / narrow-end;
		flex-direction: column;
		padding: 28px;
		overflow: hidden;
		gap: 32px;
		border-radius: var(--radius-xl);
		background-color: var(--clr-scale-ntrl-20);
		color: var(--clr-scale-ntrl-100);
	}

	.nightly-hero__header {
		display: flex;
		gap: 24px;
	}

	.nightly-hero__header-icon {
		width: 80px;
		height: 80px;
	}

	.nightly-hero__header-labels {
		display: flex;
		flex-direction: column;
		font-family: var(--fontfamily-mono);
	}

	.nightly-hero__header-labels h1 {
		margin: 0;
		margin-bottom: 6px;
		font-size: 48px;
		line-height: 1;
		font-family: var(--fontfamily-accent);
	}

	.nightly-hero__header-details {
		display: inline-flex;
		flex-wrap: wrap;
		align-items: center;
		margin-bottom: 16px;
		gap: 8px;
		font-size: 13px;
		opacity: 0.6;
	}

	.nightly-hero__description {
		max-width: 600px;
		font-size: 13px;
		line-height: 1.5;
	}

	.latest-nightly {
		display: flex;
		position: relative;
		flex-direction: column;
		padding: 30px 0 40px;
		gap: 28px;

		&::after,
		&::before {
			z-index: 0;
			position: absolute;
			right: 0;
			left: 0;
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

		&::after {
			bottom: 0;
		}
		&::before {
			top: 0;
		}
	}

	.latest-nightly__header {
		font-size: 40px;
		line-height: 1;
		font-family: var(--fontfamily-accent);

		& .arrow-down {
			display: inline-block;
			width: 34px;
			height: 33px;
			transform: translateY(8px);
		}
	}

	.download-links {
		display: flex;
		flex-wrap: wrap;
		gap: 24px;
	}

	.download-category {
		display: flex;
		flex-direction: column;
		min-width: 180px;
		gap: 16px;
	}

	.download-icon {
		width: 22px;
		height: 22px;
		opacity: 0.6;
	}

	.download-options {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.download-link {
		text-decoration: underline;
		text-underline-offset: 2px;
		transition: all 0.1s ease;

		&:hover {
			text-decoration: underline wavy;
			text-decoration-color: var(--clr-theme-pop-element);
		}
	}

	.nightly-warning {
		color: var(--clr-theme-warn-soft);
		font-size: 13px;
		line-height: 1.5;
		font-family: var(--fontfamily-mono);

		& a {
			color: var(--clr-theme-warn);
			text-decoration: underline;
			text-underline-offset: 2px;

			&:hover {
				text-decoration: underline wavy;
				text-decoration-color: var(--clr-theme-warn);
			}
		}
	}

	.releases {
		display: flex;
		grid-column: narrow-start / narrow-end;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-xl);
		font-family: var(--fontfamily-mono);

		& h3 {
			padding: 16px 24px 12px;
			font-size: 40px;
			font-family: var(--fontfamily-accent);
		}
	}

	.release-row {
		border-bottom: 1px solid var(--clr-border-2);

		&:last-child {
			border-bottom: none;
		}
	}

	.release-row__button {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		padding: 12px 24px;
		border: none;
		background: none;
		text-align: left;
		cursor: pointer;
		transition: background-color 0.1s ease;

		&:hover {
			background-color: var(--clr-bg-1);
		}
	}

	.release-row__version {
		font-size: 18px;
	}

	.release-row__date {
		color: var(--clr-text-2);
		font-size: 14px;
	}

	.release-row__links {
		display: flex;
		flex-wrap: wrap;
		padding: 24px;
		gap: 18px 20px;
		background-image: radial-gradient(var(--clr-border-2) 1px, transparent 1px);
		background-size: 6px 6px;
		font-size: 13px;

		& .download-link {
			background-color: var(--clr-bg-2);
		}
	}

	/* HERO BACKGROUND */
	.background__noisy {
		position: absolute;
		width: 500px;
		height: 240px;
		transform: scale(3) rotate(-25deg);
		border-radius: 50%;
		background:
			radial-gradient(ellipse at 50% 50%, rgba(0, 0, 0, 1), rgba(0, 0, 0, 0)),
			url("data:image/svg+xml,%3Csvg viewBox='0 0 600 600' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='noiseFilter'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.65' numOctaves='3' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23noiseFilter)'/%3E%3C/svg%3E");
		mix-blend-mode: screen;
		filter: contrast(145%) brightness(1050%) invert(100%);
		opacity: 0.2;
		pointer-events: none;

		&.noisy-1 {
			top: -20%;
			left: -20%;
		}

		&.noisy-2 {
			right: -20%;
			bottom: -10%;
		}
	}

	@media (--mobile-viewport) {
		.nightly-hero {
			padding: 20px;
		}

		.nightly-hero__header {
			flex-direction: column;
			gap: 16px;
		}

		.nightly-hero__header-labels h1 {
			font-size: 36px;
		}

		.latest-nightly__header {
			font-size: 32px;

			& .arrow-down {
				width: 28px;
				height: 28px;
				transform: translateY(6px);
			}
		}

		.download-category {
			min-width: 140px;
		}

		.release-row__button {
			padding: 12px 16px;
		}

		.release-row__version {
			font-size: 16px;
		}

		.release-row__links {
			padding: 16px;
		}
	}
</style>
