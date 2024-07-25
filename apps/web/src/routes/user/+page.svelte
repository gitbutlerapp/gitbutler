<script lang="ts">
	import { onMount } from 'svelte';
	import { env } from '$env/dynamic/public';

	let state = 'loading';
	let user: any = {};

	onMount(() => {
		let key = localStorage.getItem('gb_access_token');
		if (key) {
			fetch(env.PUBLIC_APP_HOST + 'api/user', {
				method: 'GET',
				headers: {
					'X-AUTH-TOKEN': key || ''
				}
			})
				.then(async (response) => await response.json())
				.then((data) => {
					console.log(data);
					user = data;
					state = 'loaded';
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
	{user.name}
	<div>{user.email}</div>
	<img alt="User Avatar" width="50" src={user.avatar_url} />
	{user.created_at}
	{user.supporter}
{/if}
