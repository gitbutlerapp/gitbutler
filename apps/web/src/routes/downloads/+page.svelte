<script lang="ts">
	import { marked } from 'marked';
	import { onMount } from 'svelte';

	let loading = $state(true);
	let releases: any[] = $state([]);
	let nightlies: any[] = $state([]);

	onMount(() => {
		fetch('https://gitbutler.com/api/downloads?limit=40')
			.then(async (response) => await response.json())
			.then((data) => {
				console.log(data);
				releases = data.filter((release: any) => release.channel === 'release');
				nightlies = data.filter((release: any) => release.channel === 'nightly');
				loading = false;
			});
	});
</script>

<svelte:head>
	<title>GitButler | Downloads</title>
</svelte:head>

{#if loading}
	<p>Loading...</p>
{:else}
	<div class="releases">
		<div class="release-lane">
			<h2>Stable Releases</h2>
			{#each releases as release}
				<div class="release">
					<div class="release__version">
						Version: <b>{release.version}</b> <span class="release__sha">{release.sha.substring(0, 6)}</span>
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
			{#each nightlies as release}
				<div class="release">
					<div class="release__version">
						Version: <b>{release.version}</b> <span class="release__sha">{release.sha.substring(0, 6)}</span>
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
{/if}

<style>
	h2 {
		margin-bottom: 1rem;
		font-size: 1.5rem;
	}

	hr {
		margin-block: 1rem;
		opacity: 0.25;
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
		border: 1px solid #cccccca9;
		border-radius: 0.5rem;
	}

</style>
