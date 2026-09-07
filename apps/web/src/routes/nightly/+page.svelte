<script lang="ts">
	import Footer from "$lib/components/marketing/Footer.svelte";
	import Header from "$lib/components/marketing/Header.svelte";
	import ReleaseDownloadLinks from "$lib/components/marketing/ReleaseDownloadLinks.svelte";
	import { Icon } from "@gitbutler/ui";
	import { untrack } from "svelte";
	import type { Release } from "$lib/types/releases";
	import type { LatestReleaseBuilds } from "$lib/utils/releaseUtils";
	import type { PageData } from "./$types";

	interface Props {
		data: {
			otherNightlies: Release[];
			latestNightly: Release | null;
			latestNightlyBuilds: LatestReleaseBuilds;
			nextNightly: PageData["nextNightly"];
		};
	}

	const { data }: Props = $props();

	const { latestNightly, latestNightlyBuilds, otherNightlies } = untrack(() => data);
	let nextLinuxArch = $state<"x86-64" | "ARM64">("x86-64");
	const nextPlatforms = $derived([
		{ name: "macOS", arch: "Apple Silicon", build: data.nextNightly.mac },
		{
			name: "Linux",
			arch: nextLinuxArch,
			build: nextLinuxArch === "ARM64" ? data.nextNightly.linuxArm64 : data.nextNightly.linux,
		},
	]);

	let linuxArch = $state<"x86-64" | "ARM64">("x86-64");
	let expandedRelease: string | null = $state(null);

	function toggleRelease(version: string) {
		expandedRelease = expandedRelease === version ? null : version;
	}
</script>

<svelte:head>
	<title>GitButler | Nightly Builds</title>
</svelte:head>

<section class="latest-nightly-wrapper">
	<Header />

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
							>{new Date(latestNightly.released_at).toLocaleDateString("en-GB", {
								day: "numeric",
								month: "long",
								year: "numeric",
							})} at {new Date(latestNightly.released_at).toLocaleTimeString("en-GB", {
								hour: "2-digit",
								minute: "2-digit",
								hour12: false,
							})}
						</span>
						<span> • </span>
						<a
							href="https://github.com/gitbutlerapp/gitbutler/commit/{latestNightly.sha}"
							target="_blank"
							rel="noopener noreferrer"
							class="sha-link"
						>
							{latestNightly.sha.substring(0, 7)}
						</a>
					</div>
					<p class="nightly-hero__description">
						Experience GitButler's newest features before anyone else. Nightly builds are
						automatically generated from the latest development code and may contain experimental
						features and bugs.
					</p>
				</div>
			</div>

			<div class="download-links__wrapper">
				<div class="download-card">
					<svg
						width="27"
						height="33"
						viewBox="0 0 27 33"
						class="download-card-logo"
						xmlns="http://www.w3.org/2000/svg"
						fill="currentColor"
					>
						<path
							d="M19.8726 0C19.9497 0 20.0268 0 20.1082 0C20.2973 2.32723 19.4058 4.06613 18.3224 5.32539C17.2593 6.57595 15.8036 7.78883 13.4491 7.6048C13.292 5.31089 14.185 3.70096 15.2669 2.44461C16.2704 1.27375 18.11 0.231854 19.8726 0Z"
						/>
						<path
							d="M27 24.2229C27 24.2461 27 24.2664 27 24.2881C26.3383 26.2849 25.3945 27.9963 24.2427 29.5845C23.1913 31.0263 21.9028 32.9667 19.6021 32.9667C17.6141 32.9667 16.2937 31.6929 14.2562 31.6581C12.101 31.6234 10.9158 32.7232 8.94522 33C8.71981 33 8.4944 33 8.27335 33C6.82635 32.7913 5.65857 31.6495 4.80782 30.6206C2.2992 27.5804 0.360659 23.6534 0 18.628C0 18.1353 0 17.6441 0 17.1514C0.152698 13.5547 1.90655 10.6305 4.23775 9.21328C5.46806 8.45975 7.15938 7.81781 9.04266 8.10473C9.84978 8.22935 10.6744 8.50468 11.3971 8.7771C12.0821 9.03939 12.9387 9.50454 13.7501 9.47991C14.2998 9.46397 14.8467 9.1785 15.4007 8.97708C17.0237 8.3931 18.6147 7.72362 20.7117 8.03807C23.232 8.41773 25.0207 9.53353 26.126 11.255C23.994 12.607 22.3085 14.6444 22.5965 18.1237C22.8524 21.2842 24.6964 23.1332 27 24.2229Z"
						/>
					</svg>

					<div class="stack-v gap-6">
						<a
							class="download-card-title download-card-link"
							href={latestNightlyBuilds.darwin_aarch64?.url ?? ""}>Apple Silicon</a
						>
						<span class="download-card-subtile"
							>or <a class="download-card-link" href={latestNightlyBuilds.darwin_x86_64?.url ?? ""}
								>intel-based</a
							></span
						>
					</div>
				</div>

				<div class="download-card download-card__linux">
					<div class="download-card__linux-top">
						<svg
							width="28"
							height="28"
							viewBox="0 0 28 28"
							fill="currentColor"
							xmlns="http://www.w3.org/2000/svg"
						>
							<path
								d="M18.4764 27.1234C18.7028 27.4671 18.4676 28.0009 18.0304 27.9998H9.97285C9.54678 28.0009 9.29537 27.4748 9.52693 27.1234C11.613 23.8928 16.3902 23.8928 18.4764 27.1234ZM26.8964 27.9998H21.6207C21.3989 27.9995 21.189 27.8458 21.1142 27.6288C18.7842 20.7832 9.11483 21.0894 6.88911 27.6288C6.81425 27.8458 6.60438 27.9995 6.38256 27.9998H1.09885C0.167312 28.016 -0.313736 26.8807 0.225868 26.1798C3.61303 22.0733 4.30114 15.0905 4.30114 10.0799C4.30114 4.75607 8.55683 0 14.001 0C19.4451 0 23.7008 4.75607 23.7008 10.0799C23.7008 15.3131 24.5113 21.8188 27.7774 26.1826C28.326 26.9 27.7954 28.0155 26.8964 27.9998ZM8.61218 11.7599C8.61218 12.7678 9.54103 13.615 10.5442 13.4076C11.1714 13.278 11.6896 12.7394 11.8144 12.0877C12.0038 11.0979 11.2537 10.0799 10.2288 10.0799C9.32146 10.0799 8.61218 10.8726 8.61218 11.7599ZM19.2766 16.2987C19.021 15.7675 18.3424 15.5321 17.8311 15.7975L14.001 17.7883L10.1722 15.7975C9.66058 15.532 8.98148 15.7677 8.72602 16.2994C8.47055 16.8311 8.69733 17.5368 9.20899 17.8023L13.52 20.0423C13.8191 20.1975 14.1842 20.1975 14.4833 20.0423L18.7943 17.8023C19.3281 17.5254 19.5325 16.8296 19.2766 16.2987ZM19.3898 11.7599C19.3898 10.7636 18.475 9.90195 17.4577 10.1122C16.8305 10.2419 16.3123 10.7804 16.1876 11.4322C16.0001 12.4117 16.7351 13.4399 17.7731 13.4399C18.6805 13.4399 19.3898 12.6472 19.3898 11.7599Z"
							/>
						</svg>

						<select class="linux-arch-select" bind:value={linuxArch}>
							<option value="x86-64">x86-64</option>
							<option value="ARM64">ARM64</option>
						</select>
					</div>

					<div class="stack-v gap-6">
						<div class="flex gap-16">
							<a
								class="download-card-title download-card-link"
								href={linuxArch === "x86-64"
									? (latestNightlyBuilds.linux_deb_x86_64?.url ?? "")
									: (latestNightlyBuilds.linux_deb_aarch64?.url ?? "")}>.DEB</a
							>
							<a
								class="download-card-title download-card-link"
								href={linuxArch === "x86-64"
									? (latestNightlyBuilds.linux_rpm_x86_64?.url ?? "")
									: (latestNightlyBuilds.linux_rpm_aarch64?.url ?? "")}>.RPM</a
							>
						</div>

						<span class="download-card-small-subtile"
							>Not working? Have a look at <a
								class="download-card-link"
								href="https://github.com/gitbutlerapp/gitbutler/blob/master/LINUX.md"
								target="_blank"><i>our docs</i></a
							></span
						>

						{#if linuxArch === "x86-64" ? latestNightlyBuilds.linux_cli_x86_64?.url : latestNightlyBuilds.linux_cli_aarch64?.url}
							<div class="download-card-cli-link">
								<svg width="20" height="18" viewBox="0 0 20 18" xmlns="http://www.w3.org/2000/svg">
									<path
										opacity="0.5"
										fill="currentColor"
										d="M16 0C18.2091 0 20 1.79086 20 4V14C20 16.2091 18.2091 18 16 18H4L3.79395 17.9951C1.68056 17.8879 0 16.14 0 14V4C0 1.79086 1.79086 8.45489e-08 4 0H16ZM4 1.5C2.61929 1.5 1.5 2.61929 1.5 4V14C1.5 15.3807 2.61929 16.5 4 16.5H16C17.3807 16.5 18.5 15.3807 18.5 14V4C18.5 2.61929 17.3807 1.5 16 1.5H4ZM15.3359 14.0156H10.0029V12.5156H15.3359V14.0156ZM10.4756 8.34961L11.1914 8.93164L10.4756 9.51367L5.1416 13.8477L4.19629 12.6836L8.8125 8.93164L4.19629 5.18066L5.1416 4.0166L10.4756 8.34961Z"
									/>
								</svg>
								<a
									href={linuxArch === "x86-64"
										? (latestNightlyBuilds.linux_cli_x86_64?.url ?? "")
										: (latestNightlyBuilds.linux_cli_aarch64?.url ?? "")}>Download CLI binary</a
								>
							</div>
						{/if}
					</div>
				</div>

				<div class="download-card">
					<svg
						width="28"
						height="28"
						viewBox="0 0 28 28"
						class="download-card-logo"
						xmlns="http://www.w3.org/2000/svg"
						fill="currentColor"
					>
						<path
							d="M9.81457 3.28266L0 5.05333V13.2454L9.81451 13.096L9.81457 3.28266ZM28 15.18L12.0603 14.9333V25.1213L28 28V15.18ZM9.81457 14.9027L6.84509e-05 14.752V22.9427L9.81457 24.7147V14.9027ZM28 0L12.0603 2.876V13.064L28 12.8187V0Z"
						/>
					</svg>

					<div class="stack-v gap-8">
						<a
							class="download-card-title download-card-link"
							href={latestNightlyBuilds.windows_x86_64?.url ?? ""}>Windows (MSI)</a
						>
					</div>
				</div>
			</div>

			<p class="nightly-warning">
				⚠️ Nightly builds are experimental and may be unstable. Use at your own risk.
				<br />
				For production use, please use the
				<a href="/downloads">stable release</a>.
			</p>
		</div>
	{:else}
		<div class="no-nightly">
			<p class="text-16 clr-text-2">No nightly builds are currently available.</p>
		</div>
	{/if}
</section>

<section class="next-nightly" aria-labelledby="next-nightly-title">
	<div class="next-nightly__heading">
		<div>
			<span class="next-nightly__eyebrow">A look ahead</span>
			<h2 id="next-nightly-title">GitButler <i>Next</i> Nightly</h2>
		</div>
		<span class="next-nightly__badge">Early preview</span>
	</div>
	<p class="next-nightly__description">
		Try the next generation of GitButler. Fresh builds every night, with features still taking
		shape. Expect rough edges.
	</p>
	<div class="next-nightly__downloads">
		{#each nextPlatforms as platform (platform.name)}
			<div class="next-nightly__platform">
				<div class="next-nightly__platform-heading">
					<h3>{platform.name}</h3>
					{#if platform.name === "Linux"}
						<select
							class="next-nightly__arch-select"
							aria-label="GitButler Next Linux architecture"
							bind:value={nextLinuxArch}
						>
							<option value="x86-64">x86-64</option>
							<option value="ARM64">ARM64</option>
						</select>
					{:else}
						<span>{platform.arch}</span>
					{/if}
				</div>
				{#if platform.build}
					<div class="next-nightly__links">
						{#each platform.build.downloads as download (download.url)}
							<a
								href={download.url}
								aria-label={`Download GitButler Next for ${platform.name} ${platform.arch} (${download.label})`}
							>
								{download.label} <span aria-hidden="true">↗</span>
							</a>
						{/each}
					</div>
					<span class="next-nightly__version">v{platform.build.version}</span>
				{:else}
					<p class="next-nightly__status">Downloads temporarily unavailable</p>
				{/if}
			</div>
		{/each}
		<div class="next-nightly__platform next-nightly__platform--soon">
			<div class="next-nightly__platform-heading"><h3>Windows</h3></div>
			<span class="next-nightly__version">Coming soon</span>
		</div>
	</div>
</section>

{#if otherNightlies.length > 0}
	<section class="releases">
		<h2>
			Other <i>nightly</i> builds:
		</h2>

		{#each otherNightlies as release, index (`${release.version}-${release.sha}-${index}`)}
			<div class="release-row">
				<button
					type="button"
					class="release-row__button"
					class:expanded={expandedRelease === release.version}
					onclick={() => toggleRelease(release.version)}
				>
					<div class="release-row__chevron" class:expanded={expandedRelease === release.version}>
						<Icon name="chevron-right" />
					</div>
					<div class="release-row__content">
						<span class="release-row__version">{release.version}</span>
						<div class="release-row__info">
							<span class="release-row__date">
								{new Date(release.released_at).toLocaleDateString("en-GB", {
									day: "numeric",
									month: "short",
									year: "numeric",
								})},
								{new Date(release.released_at).toLocaleTimeString("en-GB", {
									hour: "2-digit",
									minute: "2-digit",
									hour12: false,
								})},
							</span>
							<div class="flex items-center gap-2">
								<span class="release-row__separator">#</span>
								<a
									href="https://github.com/gitbutlerapp/gitbutler/commit/{release.sha}"
									target="_blank"
									rel="noopener noreferrer"
									title="View Commit on GitHub"
									class="sha-link"
									onclick={(e) => e.stopPropagation()}
								>
									{release.sha.substring(0, 7)}
								</a>
							</div>
						</div>
					</div>
				</button>

				{#if expandedRelease === release.version}
					{#if release.builds && release.builds.length > 0}
						<ReleaseDownloadLinks builds={release.builds} />
					{/if}
				{/if}
			</div>
		{/each}
	</section>
{/if}

<Footer showDownloadLinks={false} />

<style>
	.next-nightly {
		grid-column: narrow-start / narrow-end;
		padding: 28px;
		border: 1px solid #479fe8;
		border-radius: var(--radius-xl);
		background:
			radial-gradient(ellipse at top right, #1988ff66, transparent 65%),
			linear-gradient(120deg, #064a86, #007acf);
		color: #f0f8ff;
	}

	.next-nightly__heading,
	.next-nightly__platform-heading {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
	}

	.next-nightly__eyebrow,
	.next-nightly__badge,
	.next-nightly__version,
	.next-nightly__platform-heading span {
		color: #c5e3fa;
		font-size: 12px;
		font-family: var(--font-mono);
	}

	.next-nightly__eyebrow {
		letter-spacing: 0.08em;
		text-transform: uppercase;
	}

	.next-nightly__arch-select {
		padding: 3px 6px;
		border: 1px solid #8ac4f066;
		border-radius: 8px;
		background-color: #007acf;
		color: #c5e3fa;
		font-size: 12px;
		font-family: var(--font-mono);
		cursor: pointer;

		&:focus-visible {
			outline: 2px solid #c0e4ff;
			outline-offset: 3px;
		}
	}

	.next-nightly__heading {
		align-items: baseline;
	}

	.next-nightly__version {
		margin-top: auto;
	}

	.next-nightly h2 {
		margin: 6px 0 0;
		font-size: 36px;
		line-height: 1.15;
		font-family: var(--font-accent);
	}

	.next-nightly h2 i {
		color: #c0e4ff;
	}

	.next-nightly__badge {
		padding: 6px 10px;
		border: 1px solid #8ac4f066;
		border-radius: 100px;
	}

	.next-nightly__description {
		max-width: 620px;
		margin: 14px 0 22px;
		color: #d3e9fa;
		font-size: 14px;
		line-height: 1.5;
	}

	.next-nightly__downloads {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 12px;
	}

	.next-nightly__platform {
		display: flex;
		flex-direction: column;
		padding: 18px;
		gap: 14px;
		border: 1px solid #9acff54d;
		border-radius: 12px;
		background: #ffffff08;
	}

	.next-nightly__platform h3 {
		margin: 0;
		font-weight: 600;
		font-size: 16px;
	}

	.next-nightly__links {
		display: flex;
		flex-wrap: wrap;
		gap: 10px 18px;
	}

	.next-nightly__links a {
		color: #f0f8ff;
		font-weight: 600;
		font-size: 14px;
		text-decoration: underline;
		text-underline-offset: 4px;

		&:hover {
			color: #c0e4ff;
		}

		&:focus-visible {
			border-radius: 2px;
			outline: 2px solid #c0e4ff;
			outline-offset: 5px;
		}
	}

	.next-nightly__platform--soon {
		border-style: dashed;
		background: transparent;
	}

	.next-nightly__status {
		color: #c5e3fa;
		font-size: 14px;
	}

	@media (max-width: 700px) {
		.next-nightly {
			padding: 20px;
		}

		.next-nightly h2 {
			font-size: 30px;
		}

		.next-nightly__downloads {
			grid-template-columns: 1fr;
		}
	}

	.latest-nightly-wrapper {
		display: grid;
		grid-template-columns: subgrid;
		row-gap: 30px;
		grid-column: full-start / full-end;
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
		background-color: var(--fill-gray-bg);
		color: var(--fill-gray-fg);
	}

	.nightly-hero__header {
		display: flex;
		gap: 24px;
	}

	.nightly-hero__header-icon {
		z-index: var(--z-ground);
		width: 80px;
		height: 80px;
	}

	.nightly-hero__header-labels {
		display: flex;
		flex-direction: column;
		font-family: var(--font-mono);
	}

	.nightly-hero__header-labels h1 {
		margin: 0;
		margin-bottom: 6px;
		font-size: 48px;
		line-height: 1;
		font-family: var(--font-accent);
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

	/* LINKS */
	.download-links__wrapper {
		display: flex;
		position: relative;
		padding: 10px 0 40px;
		gap: 10px;

		&::after {
			z-index: 0;
			position: absolute;
			right: 0;
			bottom: 0;
			left: 0;
			height: 1px;
			background: repeating-linear-gradient(
				to right,
				var(--text-2),
				var(--text-2) 2px,
				transparent 2px,
				transparent 6px
			);
			content: "";
			pointer-events: none;
		}
	}

	.download-card {
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		justify-content: space-between;
		width: 100%;
		padding: 24px;
		overflow: hidden;
		gap: 16px;
		border: 1px solid var(--text-2);
		border-radius: var(--radius-xl);
		color: var(--fill-gray-fg);
	}

	.download-card__linux {
		flex: 1.2;
	}

	.download-card__linux-top {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
	}

	.linux-arch-select {
		appearance: none;
		position: absolute;
		top: 16px;
		right: 16px;
		padding: 3px 24px 3px 6px;
		border: 1px solid var(--text-2);
		border-radius: 8px;
		background-image: url("data:image/svg+xml,%3Csvg width='10' height='6' viewBox='0 0 10 6' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cpath d='M1 1L5 5L9 1' stroke='%23888' stroke-width='1.5' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E");
		background-position: right 8px center;
		background-repeat: no-repeat;
		color: var(--fill-gray-fg);
		font-size: 13px;
		font-family: var(--font-mono);
		cursor: pointer;
		transition: border-color 0.1s ease;

		&:hover {
			border-color: var(--text-3);
		}

		&:focus {
			border-color: var(--text-3);
			outline: none;
		}
	}

	.download-card-link {
		text-decoration: underline;
		text-underline-offset: 2px;

		&:hover {
			text-decoration: underline wavy;
			text-decoration-color: var(--fill-pop-bg);
		}
	}

	.download-card-title {
		font-weight: 600;
		font-size: 18px;
	}

	.download-card-subtile {
		color: var(--text-2);
		font-size: 14px;
	}

	.download-card-small-subtile {
		color: var(--text-3);
		font-size: 12px;
	}

	.download-card-cli-link {
		display: flex;
		align-items: center;
		margin: 10px -24px -24px -24px;
		padding: 16px 22px 18px 22px;
		gap: 8px;
		background-color: var(--text-1);

		& a {
			font-size: 14px;
			text-decoration: underline;

			&:hover {
				text-decoration: underline wavy;
				text-decoration-color: var(--fill-pop-bg);
			}
		}
	}

	.nightly-warning {
		color: var(--chip-warn-bg);
		font-size: 13px;
		line-height: 1.5;
		font-family: var(--font-mono);

		& a {
			color: var(--fill-warn-bg);
			text-decoration: underline;
			text-underline-offset: 2px;

			&:hover {
				text-decoration: underline wavy;
				text-decoration-color: var(--fill-warn-bg);
			}
		}
	}

	.releases {
		display: flex;
		grid-column: narrow-start / narrow-end;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-xl);
		font-family: var(--font-mono);

		& h2 {
			padding: 16px 24px 12px;
			font-size: 40px;
			line-height: 1.2;
			font-family: var(--font-accent);
		}
	}

	.release-row {
		border-bottom: 1px solid var(--border-2);

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
		transition: background-color 0.1s ease;

		&:hover {
			background-color: var(--bg-1);
		}
	}

	.release-row__chevron {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		margin-right: 8px;
		color: var(--text-3);
		transition: transform 0.15s ease;

		&.expanded {
			transform: rotate(90deg);
		}
	}

	.release-row__content {
		display: flex;
		flex: 1;
		align-items: center;
		justify-content: space-between;
	}

	.release-row__version {
		font-size: 18px;
	}

	.release-row__info {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
	}

	.release-row__date {
		color: var(--text-2);
		font-size: 14px;
	}

	.release-row__separator {
		color: var(--text-3);
		font-size: 14px;
	}

	.sha-link {
		color: var(--text-3);
		font-size: 14px;
		text-decoration: underline;
		text-underline-offset: 2px;
		transition: all 0.1s ease;

		&:hover {
			color: var(--fill-pop-bg);
			text-decoration: underline wavy;
			text-decoration-color: var(--fill-pop-bg);
		}
	}

	@media (max-width: 900px) {
		.download-links__wrapper {
			flex-direction: column;
		}

		.download-card {
			flex: auto;
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

		.release-row__button {
			align-items: flex-start;
			padding: 12px 16px;
		}

		.release-row__chevron {
			margin-top: 4px;
		}

		.release-row__content {
			flex-direction: column;
			align-items: flex-start;
			width: 100%;
		}

		.release-row__version {
			font-size: 16px;
		}
	}
</style>
