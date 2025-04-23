<script lang="ts">
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

	const { nightlies, releases, latestRelease, latestReleaseBuilds } = data;
</script>

<svelte:head>
	<title>GitButler | Downloads</title>
</svelte:head>

<div class="downloads">
	<h1>Latest Release</h1>
	<div class="current-release">
		<div class="current-release__group">
			<div>
				<img src="/images/icon.png" width="200px" alt="GitButler" />
			</div>
			<div class="current-builds">
				<div class="version">
					<div class="current__version">{latestRelease?.version}</div>
					<div class="current__version-date">{latestRelease?.released_at.substring(0, 10)}</div>
				</div>
				<div class="current-builds-group">
					<div>
						<div class="os windows">
							<div class="os__name">
								<img
									class="os-select__section-os-icon"
									src="/images/os-icons/windows-small-logo.svg"
									alt=""
								/>
								Windows
							</div>
							<div class="os__downloads">
								{#if latestReleaseBuilds?.['windows_x86_64']}
									<a href={latestReleaseBuilds['windows_x86_64'].url}>Download Windows (MSI)</a>
								{/if}
							</div>
						</div>
						<div class="os apple">
							<div class="os__name">
								<img
									class="os-select__section-os-icon"
									src="/images/os-icons/apple-small-logo.svg"
									alt=""
								/>
								macOS
							</div>
							<div class="os__downloads">
								{#if latestReleaseBuilds?.['darwin_x86_64']}
									<a href={latestReleaseBuilds['darwin_x86_64'].url}>Download Intel</a>
								{/if}
								{#if latestReleaseBuilds?.['darwin_aarch64']}
									<a href={latestReleaseBuilds['darwin_aarch64'].url}>Download Apple Silicon</a>
								{/if}
							</div>
						</div>
					</div>
					<div>
						<div class="os linux">
							<div class="os__name">
								<img
									class="os-select__section-os-icon"
									src="/images/os-icons/linux-small-logo.svg"
									alt=""
								/>
								Linux
							</div>
							<div class="os__downloads">
								{#if latestReleaseBuilds?.['linux_appimage']}
									<a href={latestReleaseBuilds['linux_appimage'].url}>Download AppImage</a>
								{/if}
								{#if latestReleaseBuilds?.['linux_deb']}
									<a href={latestReleaseBuilds['linux_deb'].url}>Download Deb</a>
								{/if}
								{#if latestReleaseBuilds?.['linux_rpm']}
									<a href={latestReleaseBuilds['linux_rpm'].url}>Download RPM</a>
								{/if}
							</div>
						</div>
					</div>
				</div>
			</div>
		</div>
	</div>

	<h1>All Recent Releases</h1>
	<div class="releases">
		<div class="release-lane">
			<h2>Stable Releases</h2>
			{#each releases as release}
				<div class="release">
					<div class="release__version">
						Version: <b>{release.version}</b>
						<span class="release__sha">{release.sha.substring(0, 6)}</span>
					</div>
					<div>Released: {new Date(release.released_at).toLocaleString()}</div>
					{#if release.notes}
						<div class="release__notes dotted">{@html marked(release.notes)}</div>
					{/if}
					<div class="builds">
						{#each release.builds as build}
							<li><a class="linked" href={build.url}>{build.platform}</a></li>
						{/each}
					</div>
				</div>
				<hr />
			{/each}
		</div>
		<div class="release-lane">
			<h2>Nightly Releases</h2>
			<div class="nightly-warning">
				These are nightly builds that are automatically built from the master branch each night and
				may be unstable.
			</div>
			{#each nightlies as release}
				<div class="release">
					<div class="release__version">
						Version: <b>{release.version}</b>
						<span class="release__sha">{release.sha.substring(0, 6)}</span>
					</div>
					<div>Released: {new Date(release.released_at).toLocaleString()}</div>
					{#if release.notes}
						<div class="release__notes dotted">{@html marked(release.notes)}</div>
					{/if}
					<div class="builds">
						{#each release.builds as build}
							<li><a class="linked" href={build.url}>{build.platform}</a></li>
						{/each}
					</div>
				</div>
				<hr />
			{/each}
		</div>
	</div>
</div>

<style>
	h1 {
		font-size: 2rem;
		margin-bottom: 1rem;
	}

	h2 {
		margin-bottom: 1rem;
		font-size: 1.5rem;
	}

	hr {
		margin-block: 1rem;
		opacity: 0.25;
	}

	.current-release {
		display: flex;
		flex-direction: row;
		gap: 32px;
		margin-bottom: 60px;
		padding-bottom: 60px;
		border-bottom: 1px solid #ccc;
		border-style: dashed;
	}

	.current-builds {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.current-builds-group {
		display: flex;
		flex-direction: row;
		gap: 1.8rem;
	}

	.version {
		display: flex;
		margin-top: 18px;
		flex-direction: row;
		align-items: baseline;
		gap: 12px;
	}

	.current__version {
		font-size: 1.5rem;
	}

	.current__version-date {
		font-size: 0.8rem;
		color: #777;
	}

	.current-release__group {
		display: flex;
		flex-direction: row;
		gap: 32px;
	}

	.downloads {
		padding: 20px 30px;
	}

	.os {
		display: flex;
		flex-direction: column;
		gap: 8px;
		margin-bottom: 1.5rem;
	}

	.os__name {
		display: flex;
		flex-direction: row;
		align-items: center;
		gap: 8px;
		color: #777;
	}

	.os__downloads {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 8px;
	}

	.os__downloads a {
		color: #44c;
		text-decoration: underline;
	}

	.builds {
		margin-block: 0.25rem;
	}

	.builds > * {
		line-height: 1.5;
	}

	.release__notes {
		margin-block: 0.5rem;
		background-color: #ddd;
		padding: 1rem;
		border-radius: 0.5rem;
		color: #444;
		font-size: 0.9rem;
	}

	.release__version {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.release__sha {
		opacity: 0.5;
	}

	.releases {
		display: flex;
		flex-wrap: nowrap;
		gap: 1rem;
	}

	.release-lane {
		display: flex;
		flex-direction: column;
		width: calc(50% - 3rem);
		margin: 1rem;
		padding: 1rem;
		border: 1px solid #ccc;
		border-radius: 0.5rem;
	}

	.nightly-warning {
		font-size: 0.8rem;
		color: #777;
		background-color: #fdd;
		border-radius: 8px;
		padding: 1rem;
		margin-bottom: 20px;
	}
</style>
