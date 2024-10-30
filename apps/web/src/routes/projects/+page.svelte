<script lang="ts">
	import { AuthService } from '$lib/auth/authService';
	import { getContext } from '@gitbutler/shared/context';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';
	import { onMount } from 'svelte';
	import { get } from 'svelte/store';
	import { env } from '$env/dynamic/public';

	dayjs.extend(relativeTime);

	let state = 'loading';
	let projects: any = {};

	const authService = getContext(AuthService);

	onMount(() => {
		const key = get(authService.token);
		if (key) {
			fetch(env.PUBLIC_APP_HOST + 'api/projects', {
				method: 'GET',
				headers: {
					'X-AUTH-TOKEN': key || ''
				}
			})
				.then(async (response) => await response.json())
				.then((data) => {
					console.log(data);
					projects = data;
					state = 'loaded';
					setTimeout(() => {
						let dtime = document.querySelectorAll('.dtime');
						dtime.forEach((element) => {
							console.log(element.innerHTML);
							element.innerHTML = dayjs(element.innerHTML).fromNow();
						});
					}, 100);
				});
		} else {
			state = 'unauthorized';
		}
	});
</script>

{#if state === 'loading'}
	<p>Loading...</p>
{:else if state === 'unauthorized'}
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
