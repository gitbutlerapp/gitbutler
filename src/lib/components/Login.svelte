<script lang="ts">
	import type { LoginToken, CloudApi, users } from '$lib/api';
	import { toasts, log } from '$lib';
	import { derived, writable } from '@square/svelte-store';
	import { open } from '@tauri-apps/api/shell';
	import Button from './Button';

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

	let signUpOrLoginLoading = false;
	const onSignUpOrLoginClick = () => {
		Promise.resolve()
			.then(() => (signUpOrLoginLoading = true))
			.then(api.login.token.create)
			.then(token.set)
			.catch((e) => {
				log.error(e);
				toasts.error('Something went wrong');
			})
			.finally(() => (signUpOrLoginLoading = false));
	};
	const token = writable<LoginToken | null>(null);
	const authUrl = derived(token, ($token) => $token?.url as string);
</script>

<div>
	{#if $user}
		<Button kind="plain" color="destructive" on:click={user.delete}>Log out</Button>
	{:else if $token !== null}
		{#await Promise.all([open($token.url), pollForUser($token.token)])}
			<div>Log in in your system browser</div>
		{/await}
		<p>
			<button class="underline" on:click={() => open($authUrl)}>Click here</button>
			if you are not redirected automatically, you can
		</p>
	{:else}
		<Button loading={signUpOrLoginLoading} color="primary" on:click={onSignUpOrLoginClick}>
			Sign up or Log in
		</Button>
	{/if}
</div>
