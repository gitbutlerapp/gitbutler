<script lang="ts">
	import type { LoginToken, CloudApi, users } from '$lib/api';
	import { derived, writable } from '@square/svelte-store';
	import { open } from '@tauri-apps/api/shell';

	export let user: Awaited<ReturnType<typeof users.CurrentUser>>;
	export let api: Awaited<ReturnType<typeof CloudApi>>;

	const pollForUser = async (token: string) => {
		const apiUser = await api.login.user.get(token).catch(() => null);
		if (apiUser) {
			user.set(apiUser);
			return apiUser;
		}
		return new Promise((resolve) => {
			setTimeout(async () => {
				resolve(await pollForUser(token));
			}, 1000);
		});
	};

	const token = writable<LoginToken | null>(null);
	const authUrl = derived(token, ($token) => $token?.url as string);
</script>

<div>
	{#if $user}
		<button class="hover:text-red-400 text-zinc-400 hover:underline" on:click={() => user.delete()}
			>Log out</button
		>
	{:else if $token !== null}
		{#await Promise.all([open($token.url), pollForUser($token.token)])}
			<div>Log in in your system browser</div>
		{/await}
		<p>
			<button class="underline" on:click={() => open($authUrl)}>Click here</button>
			if you are not redirected automatically, you can
		</p>
	{:else}
		<button
			class="rounded bg-blue-400 py-1 px-3 text-white"
			on:click={() => api.login.token.create().then(token.set)}>Sign up or Log in</button
		>
	{/if}
</div>
