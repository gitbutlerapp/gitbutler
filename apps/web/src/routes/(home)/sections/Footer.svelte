<script lang="ts">
	import * as jsonLinks from '$lib/data/links.json';
	import osIcons from '$lib/data/os-icons.json';
	import { getValidReleases } from '$lib/types/releases';
	import { clickOutside } from '@gitbutler/ui/utils/clickOutside';
	import { onMount } from 'svelte';
	import { fly } from 'svelte/transition';
	import type { Release } from '$lib/types/releases';

	let latestNightly: Release | null = null;
	let showDropdown = false;

	onMount(async () => {
		try {
			const response = await fetch(
				'https://app.gitbutler.com/api/downloads?limit=1&channel=nightly'
			);
			const data = await response.json();
			const builds = getValidReleases(data);
			latestNightly = builds.length > 0 ? builds[0] : null;
		} catch (error) {
			console.error('Failed to fetch nightly builds:', error);
		}
	});

	function toggleDropdown() {
		showDropdown = !showDropdown;
	}

	function closeDropdown() {
		showDropdown = false;
	}

	function getPlatformLabel(platform: string): string {
		const platformMap: { [key: string]: string } = {
			'darwin-aarch64': 'Apple Silicon',
			'darwin-x86_64': 'Intel Mac',
			'windows-x86_64': 'Windows',
			'linux-x86_64': 'Linux'
		};
		return platformMap[platform] || platform;
	}
</script>

<footer class="footer">
	<div class="banner">
		<div class="banner-content-downloads">
			<div class="stack-v">
				<h2 class="banner-title">Download <i>the</i> app</h2>

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
							<a href={jsonLinks.downloads.intelMac.url} class="download-link">
								{jsonLinks.downloads.intelMac.label}
							</a>
							<a href={jsonLinks.downloads.appleSilicon.url} class="download-link">
								{jsonLinks.downloads.appleSilicon.label}
							</a>
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
							<a href={jsonLinks.downloads.windowsMsi.url} class="download-link">
								{jsonLinks.downloads.windowsMsi.label}
							</a>
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
							<a href={jsonLinks.downloads.linuxAppimage.url} class="download-link"> AppImage </a>
							<a href={jsonLinks.downloads.linuxDeb.url} class="download-link"> Deb </a>
							<a href={jsonLinks.downloads.linuxRpm.url} class="download-link"> RPM </a>
						</div>
					</div>
				</div>
			</div>

			<div class="banner-nightly-text">
				<span class="opacity-50">Experience GitButler's newest features before anyone else.</span>

				<div class="nightly-dropdown" use:clickOutside={{ handler: closeDropdown }}>
					<button type="button" class="nightly-button" onclick={toggleDropdown}>
						<span class="nightly-button-label">Get Nightly</span>
						<span class="nightly-button-arrow" class:rotated={showDropdown}>▼</span>
					</button>

					{#if showDropdown && latestNightly}
						<div class="nightly-dropdown-menu" in:fly={{ y: -10, duration: 150 }}>
							{#each latestNightly.builds as platformBuild}
								<button
									type="button"
									class="nightly-platform-link"
									onclick={() => window.open(platformBuild.url, '_blank')}
								>
									<span class="nightly-download-icon">⤓</span>
									{getPlatformLabel(platformBuild.platform)}
								</button>
							{/each}
						</div>
					{/if}
				</div>
			</div>
		</div>

		<img class="banner-image" src="/images/pc-skater.svg" alt="" />

		<div class="banner-background">
			<div class="banner-background__noisy noisy-1"></div>
			<div class="banner-background__noisy noisy-2"></div>
		</div>
	</div>

	<div class="links">
		<div class="stack-v gap-12">
			<p class="text-14 opacity-40">Keep up with us:</p>
			<ul class="links-list">
				{#each Object.values(jsonLinks.resources) as resource}
					<li class="link">
						<a href={resource.url}>
							<span>{resource.label}</span>
						</a>
					</li>
				{/each}
			</ul>
		</div>

		<ul class="links-list">
			{#each Object.values(jsonLinks.social) as social}
				<li class="link">
					<a href={social.url}>
						<span>{social.label}</span>
						<svg
							class="link-arrow"
							width="14"
							height="14"
							viewBox="0 0 14 14"
							fill="none"
							xmlns="http://www.w3.org/2000/svg"
						>
							<path
								d="M1 1L13 1M13 1L13 13M13 1L1 13"
								stroke="var(--clr-black)"
								stroke-width="1.5"
							/>
						</svg>
					</a>
				</li>
			{/each}
		</ul>

		<div class="meta-links">
			<span class="meta-links__copyright"
				>©{new Date().getFullYear()} GitButler. All rights reserved.</span
			>
			<span class="meta-links__legal">
				<a href="/privacy">
					{jsonLinks.legal.privacyPolicy.label}
				</a>
				<span> | </span>
				<a href={jsonLinks.legal.termsOfService.url}>
					{jsonLinks.legal.termsOfService.label}
				</a>
			</span>
		</div>
	</div>
</footer>

<style lang="scss">
	.footer {
		display: flex;
		grid-column: full-start / full-end;
		margin-bottom: 80px;
		gap: 60px;
	}

	.banner {
		display: flex;
		position: relative;
		flex: 3;
		padding: 36px 0 0 48px;
	}

	.banner-background {
		z-index: -1;
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		overflow: hidden;
		border-radius: var(--radius-xl);
		background: var(--clr-scale-pop-60);
	}

	.banner-background__noisy {
		position: absolute;
		width: 500px;
		height: 240px;
		transform: scale(3) rotate(25deg);
		border-radius: 50%;
		background:
			radial-gradient(ellipse at 50% 50%, rgba(0, 0, 0, 1), rgba(0, 0, 0, 0)),
			url("data:image/svg+xml,%3Csvg viewBox='0 0 600 600' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='noiseFilter'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.65' numOctaves='3' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23noiseFilter)'/%3E%3C/svg%3E");
		mix-blend-mode: screen;
		filter: contrast(145%) brightness(1050%) invert(100%);
		opacity: 0.7;

		&.noisy-1 {
			bottom: -20%;
			left: -20%;
		}

		&.noisy-2 {
			top: -10%;
			right: -20%;
		}
	}

	.banner-content-downloads {
		display: flex;
		flex: 1;
		flex-direction: column;
		justify-content: space-between;
	}

	.banner-title {
		margin-bottom: 38px;
		font-size: 60px;
		line-height: 1.1;
		font-family: var(--fontfamily-accent);
	}

	.download-links {
		column-gap: 32px;
		max-width: 400px;
		columns: 2;
	}

	.download-category {
		margin-bottom: 32px;
		break-inside: avoid;
	}

	.download-category {
		display: flex;
		align-items: flex-start;
		margin-bottom: 32px;
		gap: 24px;
		break-inside: avoid;
	}

	.download-icon {
		width: 28px;
		height: 28px;
	}

	.download-options {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.download-link {
		color: var(--clr-text-1);
		font-size: 16px;
		line-height: 120%;
		text-decoration: underline;
	}

	.banner-image {
		width: 320px;
		transform: translateX(20px) translateY(10px);
	}

	.banner-nightly-text {
		display: inline;
		margin-bottom: 40px;
		font-size: 14px;
	}

	.nightly-dropdown {
		display: inline-block;
		position: relative;
	}

	.nightly-button {
		display: flex;
		align-items: center;
		border: none;
		background: none;
		color: inherit;
		cursor: pointer;
	}

	.nightly-button-label {
		text-decoration: underline dotted;
		text-underline-offset: 2px;
		cursor: pointer;
	}

	.nightly-button-arrow {
		margin-left: 4px;
		font-size: 10px;
		transition: transform var(--transition-fast);

		&.rotated {
			transform: rotate(180deg);
		}
	}

	.nightly-dropdown-menu {
		z-index: 1000;
		position: absolute;
		right: 0;
		bottom: 100%;
		min-width: 250px;
		margin-bottom: 8px;
		padding: 8px;
		border-radius: 6px;
		background: var(--clr-bg-1);
		box-shadow: var(--fx-shadow-m);
	}

	.nightly-platform-link {
		display: flex;
		align-items: center;
		width: 100%;
		padding: 8px 14px 8px 8px;
		gap: 8px;
		border: none;
		border-radius: var(--radius-s);
		background: none;
		color: inherit;
		font-size: 14px;
		text-align: left;
		text-decoration: none;
		cursor: pointer;
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-2);
			text-decoration: none;

			.nightly-download-icon {
				transform: translateY(0);
				opacity: 1;
			}
		}
	}

	.nightly-download-icon {
		display: inline-block;
		transform: translateY(-2px);
		opacity: 0;
		transition: transform 0.15s ease-in-out;
	}

	// links section
	.links {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-self: flex-end;
		gap: 24px;
	}

	.links-list {
		display: flex;
		flex-wrap: wrap;
		padding-bottom: 30px;
		gap: 16px;

		&:last-child {
			border-bottom: 1px solid var(--clr-border-2);
		}
	}

	.link {
		font-size: 14px;
		line-height: 1.3;

		a {
			display: inline-flex;
			align-items: center;

			&:hover {
				color: var(--clr-text-1);
				text-decoration: underline wavy;
				text-decoration-color: var(--clr-theme-pop-element);
			}
		}
	}

	.meta-links {
		display: flex;
		flex-direction: column;
		gap: 10px;
		color: var(--clr-text-2);
		font-size: 12px;
	}

	.meta-links__legal {
		display: flex;
		gap: 8px;

		a {
			color: inherit;
			text-decoration: underline;

			&:hover {
				color: var(--clr-text-1);
				text-decoration: none;
			}
		}
	}
</style>
