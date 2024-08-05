<script lang="ts">
	import { onMount } from 'svelte';
	import { env } from '$env/dynamic/public';

	let loading = true;
	let releases: any[] = [];
	let nightlies: any[] = [];

	onMount(() => {
		fetch(env.PUBLIC_APP_HOST + 'api/downloads?limit=40')
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
			<h2>Stable Release</h2>
			{#each releases as release}
				<div class="release">
					<div>Version: {release.version}</div>
					<div>SHA: {release.sha}</div>
					<div>{release.released_at}</div>
					<div>{release.notes}</div>
					<div class="builds">
						<h3>Builds</h3>
						{#each release.builds as build}
							<li><a href={build.url}>{build.platform}</a></li>
						{/each}
					</div>
				</div>
				<hr />
			{/each}
		</div>
		<div class="release-lane">
			<h2>Nightly Release</h2>
			{#each nightlies as release}
				<div class="release">
					<div>Version: {release.version}</div>
					<div>SHA: {release.sha}</div>
					<div>{release.released_at}</div>
					{#if release.notes}
						<div>{release.notes}</div>
					{/if}
					<div class="builds">
						<h3>Builds</h3>
						{#each release.builds as build}
							<li><a href={build.url}>{build.platform}</a></li>
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
	}

	.releases {
		display: flex;
		flex-direction: row;
	}
	.release-lane {
		margin: 1rem;
		padding: 1rem;
		border: 1px solid #ccc;
		border-radius: 0.5rem;
		width: 50%;
	}
</style>
