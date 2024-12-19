<script lang="ts">
	import { getContext } from '@gitbutler/shared/context';
	import { HttpClient } from '@gitbutler/shared/network/httpClient';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';
	import { onMount } from 'svelte';
	import type { Project } from '$lib/types';

	const httpClient = getContext(HttpClient);

	dayjs.extend(relativeTime);

	let pageState = $state('loading');
	let projects = $state<Project[]>([]);

	onMount(async () => {
		let key = localStorage.getItem('gb_access_token');
		if (key) {
			projects = await httpClient.get<Project[]>('projects', {
				headers: {
					'X-AUTH-TOKEN': key || ''
				}
			});
			pageState = 'loaded';
			setTimeout(() => {
				let dtime = document.querySelectorAll('.dtime');
				dtime.forEach((element) => {
					console.log(element.innerHTML);
					element.innerHTML = dayjs(element.innerHTML).fromNow();
				});
			}, 100);
		} else {
			pageState = 'unauthorized';
		}
	});
</script>

{#if pageState === 'loading'}
	<p>Loading...</p>
{:else if pageState === 'unauthorized'}
	<p>Unauthorized</p>
{:else}
	{#each projects as project}
		<div>
			<h2><a href="/projects/{project.repository_id}">{project.name}</a></h2>
			<p>{project.repository_id}</p>
			<p>{project.description}</p>
			<p>Created: <span class="dtime">{project.created_at}</span></p>
			<p>Updated: <span class="dtime">{project.updated_at}</span></p>
		</div>
		<hr />
	{/each}
{/if}
