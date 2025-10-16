<script lang="ts">
	import Footer from '$lib/components/marketing/Footer.svelte';
	import Header from '$lib/components/marketing/Header.svelte';
	import ReleaseCard from '$lib/components/marketing/ReleaseCard.svelte';
	import osIcons from '$lib/data/os-icons.json';
	import { marked } from '@gitbutler/ui/utils/marked';
	import type { Build, Release } from '$lib/types/releases';

	interface Props {
		data: {
			releases: Release[];
			nightlies: Release[];
			latestRelease: Release;
			latestReleaseBuilds: { [key: string]: Build };
		};
	}

	const { data }: Props = $props();

	const { latestRelease, latestReleaseBuilds } = data;
</script>

<svelte:head>
	<title>GitButler | Downloads</title>
</svelte:head>

<section class="latest-release-wrapper">
	<Header />

	<div class="latest-release">
		<div class="latest-release__header">
			<img class="latest-release__header-icon" src="/images/app-icon.svg" alt="" />

			<div class="latest-release__header-labels">
				<h1>
					{latestRelease.version}
				</h1>
				<div class="latest-release__header-subtitle">
					<span>Latest release</span>
					<span> • </span>
					<span
						>{new Date(latestRelease.released_at).toLocaleDateString('en-GB', {
							day: 'numeric',
							month: 'long',
							year: 'numeric'
						})}</span
					>
				</div>
			</div>
		</div>

		<div class="download-links__wrapper">
			<h3 class="download-links__title">
				DOWNLOAD <i>the</i> app

				<svg class="arrow-down" viewBox="0 0 34 33" fill="none" xmlns="http://www.w3.org/2000/svg">
					<path d="M23.1667 27L17.3333 20H23.1667H29L23.1667 27Z" fill="currentColor" />
					<path
						d="M4 9.5H23.1667V27M23.1667 27L17.3333 20H29L23.1667 27Z"
						stroke="currentColor"
						stroke-width="2"
					/>
				</svg>
			</h3>
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
						{#if latestReleaseBuilds.darwin_x86_64}
							<a href={(latestReleaseBuilds.darwin_x86_64 as Build).url} class="download-link">
								Intel Mac
							</a>
						{/if}
						{#if latestReleaseBuilds.darwin_aarch64}
							<a href={(latestReleaseBuilds.darwin_aarch64 as Build).url} class="download-link">
								Apple Silicon
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
						{#if latestReleaseBuilds.windows_x86_64}
							<a href={(latestReleaseBuilds.windows_x86_64 as Build).url} class="download-link">
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
						{#if latestReleaseBuilds.linux_appimage}
							<a href={(latestReleaseBuilds.linux_appimage as Build).url} class="download-link"
								>Linux Intel (AppImage)</a
							>
						{/if}
						{#if latestReleaseBuilds.linux_deb}
							<a href={(latestReleaseBuilds.linux_deb as Build).url} class="download-link">
								Linux Intel (Deb)
							</a>
						{/if}
						{#if latestReleaseBuilds.linux_rpm}
							<a href={(latestReleaseBuilds.linux_rpm as Build).url} class="download-link">
								Linux Intel (RPM)
							</a>
						{/if}
					</div>
				</div>
			</div>
		</div>

		{#if latestRelease.notes}
			<div class="release-notes-content">
				{@html marked(latestRelease.notes)}
			</div>
		{/if}

		<div class="nightly-info">
			<p class="text-14 text-body clr-text-2">
				Experience GitButler’s newest features before anyone else. ⋆˚₊
				<a href="/nightlies" class="download-link"> Get Nightly </a>
				☽˚.⋆
			</p>
		</div>

		<div class="latest-release-background__noisy noisy-1"></div>
		<div class="latest-release-background__noisy noisy-2"></div>
	</div>
</section>

<section class="releases">
	{#each data.releases.filter((release) => release.version !== latestRelease.version) as release (release.version)}
		<ReleaseCard
			{release}
			showSeparator={release !==
				data.releases.filter((r) => r.version !== latestRelease.version)[
					data.releases.filter((r) => r.version !== latestRelease.version).length - 1
				]}
			showDownloadLinks
		/>
	{/each}
</section>

<Footer showDownloadLinks={false} />

<style>
	.latest-release-wrapper {
		display: grid;
		grid-template-columns: subgrid;
		row-gap: 30px;
		grid-column: full-start / full-end;
	}

	.latest-release {
		display: flex;
		position: relative;
		grid-column: narrow-start / narrow-end;
		flex-direction: column;
		padding: 28px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-xl);
		background: var(--clr-bg-1);
	}

	.latest-release__header {
		display: flex;
		align-items: center;
		margin-bottom: 28px;
		gap: 20px;
	}

	.latest-release__header-labels {
		display: flex;
		flex-direction: column;

		& h1 {
			margin: 0;
			font-size: 45px;
			line-height: 1.2;
			font-family: var(--fontfamily-accent);
		}

		& span {
			color: var(--clr-text-2);
			font-size: 13px;
			font-family: var(--fontfamily-mono);
		}
	}

	.latest-release__header-subtitle {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px;
	}

	.release-notes-content {
		padding-bottom: 40px;
		font-size: 13px;
		font-family: var(--fontfamily-mono);
	}

	.download-links__wrapper {
		display: flex;
		position: relative;
		flex-direction: column;
		margin-bottom: 24px;
		padding: 30px 0 40px;

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

	.download-links__title {
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
		margin-top: 24px;
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
	}

	.download-options {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.download-link {
		font-size: 16px;
		text-decoration: underline;
		text-underline-offset: 2px;
		transition: all 0.1s ease;

		&:hover {
			color: var(--clr-text-1);
			text-decoration: underline wavy;
			text-decoration-color: var(--clr-theme-pop-element);
		}
	}

	.nightly-info {
		position: relative;
		padding: 24px 0 0;

		&::after {
			z-index: 0;
			position: absolute;
			top: 0;
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
	}

	.latest-release-background__noisy {
		position: absolute;
		width: 500px;
		height: 240px;
		transform: scale(3) rotate(25deg);
		border-radius: 50%;
		background:
			radial-gradient(ellipse at 50% 50%, rgba(0, 0, 0, 1), rgba(0, 0, 0, 0)),
			url("data:image/svg+xml,%3Csvg viewBox='0 0 600 600' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='noiseFilter'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.65' numOctaves='3' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23noiseFilter)'/%3E%3C/svg%3E");
		mix-blend-mode: multiply;
		filter: contrast(145%) brightness(1100%);
		opacity: 0.05;
		pointer-events: none;

		&.noisy-1 {
			bottom: -10%;
			left: -20%;
		}

		&.noisy-2 {
			top: -10%;
			right: -20%;
		}
	}

	.releases {
		display: flex;
		grid-column: narrow-start / narrow-end;
		flex-direction: column;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-xl);
		font-family: var(--fontfamily-mono);
	}

	@media (--mobile-viewport) {
		.latest-release {
			padding: 20px;
		}

		.latest-release__header {
			flex-direction: column;
			align-items: flex-start;
			gap: 12px;
		}

		.download-links__title {
			font-size: 32px;

			& .arrow-down {
				width: 28px;
				height: 27px;
				transform: translateY(6px);
			}
		}
	}
</style>
