<script lang="ts">
	import HeaderAuthSection from '$lib/components/HeaderAuthSection.svelte';
	import * as jsonLinks from '$lib/data/links.json';
	import osIcons from '$lib/data/os-icons.json';

	interface Props {
		showDownloadLinks?: boolean;
	}

	const { showDownloadLinks = true }: Props = $props();
</script>

<footer class="footer">
	<div class="banner">
		{#if showDownloadLinks}
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
					<a href="/nightlies" class="nightly-link"> Get Nightly </a>
				</div>
			</div>
		{:else}
			<div class="banner-content-downloads">
				<h2 class="banner-title">
					<i>Version</i> Control
					<br />
					With <i>Attitude</i> ⧓
				</h2>
			</div>
		{/if}

		<img class="banner-image" src="/images/pc-skater.svg" alt="" />

		<div class="banner-background">
			<div class="banner-background__noisy noisy-1"></div>
			<div class="banner-background__noisy noisy-2"></div>
		</div>
	</div>

	<div class="links">
		<ul class="links-list">
			{#each Object.values(jsonLinks.social) as social}
				<li class="text-14 link">
					<a href={social.url}>
						<span>{social.label}</span>
					</a>
				</li>
			{/each}
		</ul>

		<hr class="links-divider" />

		<ul class="links-list">
			{#each Object.values(jsonLinks.resources) as resource}
				<li class="text-16 link">
					<a href={resource.url}>
						<span>{resource.label}</span>
					</a>
				</li>
			{/each}
		</ul>

		<HeaderAuthSection hideIfUserAuthenticated />

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
		flex: 3.2;
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
		pointer-events: none;

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
		transform: translateY(-4px);
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
		text-underline-offset: 2px;

		&:hover {
			text-decoration: underline wavy;
			text-decoration-color: var(--clr-theme-pop-element);
		}
	}

	.banner-image {
		width: 320px;
		transform: translateX(20px) translateY(10px);
	}

	.banner-nightly-text {
		display: inline;
		margin-bottom: 40px;
		font-size: 14px;
		line-height: 1.6;
	}

	.nightly-link {
		color: inherit;
		text-decoration: underline;
		text-underline-offset: 3px;

		&:hover {
			color: var(--clr-text-1);
			text-decoration: underline wavy;
		}
	}

	// links section
	.links {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-self: flex-end;
		gap: 34px;
	}

	.links-divider {
		margin: 0;
		border: none;
		border-top: 1px solid var(--clr-border-2);
	}

	.links-list {
		display: flex;
		column-gap: 16px;
		row-gap: 12px;
		flex-wrap: wrap;
	}

	.link {
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

	@media (--desktop-small-viewport) {
		.footer {
			margin-bottom: 60px;
			gap: 40px;
		}

		.banner-title {
			margin-bottom: 28px;
			font-size: 52px;
		}

		.banner-image {
			width: 280px;
			transform: translateX(10px) translateY(10px);
		}

		.links {
			gap: 28px;
		}
	}

	@media (--tablet-viewport) {
		.footer {
			flex-direction: column;
			margin-bottom: 40px;
			gap: 40px;
		}

		.banner {
			padding: 24px;
		}

		.banner-title {
			margin-bottom: 24px;
			font-size: 42px;
		}

		.banner-image {
			display: none;
		}

		.banner-nightly-text {
			display: none;
		}

		.download-links {
			display: flex;
			column-gap: 40px;
			flex-wrap: wrap;
			max-width: none;
			columns: unset;
		}

		.links {
			flex-direction: row;
			flex-wrap: wrap;
			justify-content: space-between;
			gap: 16px;
		}

		.links-divider {
			display: none;
		}

		.links-list {
			align-self: flex-end;
			padding-bottom: 0;
			gap: 8px 12px;
			border-bottom: none !important;
		}

		.meta-links {
			align-self: center;
			width: 100%;
			margin-top: 20px;
			gap: 4px;
		}

		.banner-background__noisy.noisy-1 {
			bottom: -50%;
			left: -60%;
		}
		.banner-background__noisy.noisy-2 {
			top: -50%;
			right: -50%;
		}
	}

	@media (max-width: 500px) {
		.download-links {
			column-gap: 0;
			flex-direction: column;
		}
	}
</style>
