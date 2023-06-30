<script lang="ts">
	import { type LoginToken, CloudApi } from '$lib/api';
	import { toasts, log, stores } from '$lib';
	import { derived, writable } from '@square/svelte-store';
	import { open } from '@tauri-apps/api/shell';
	import Button from './Button';

	const cloud = CloudApi();
	const user = stores.user;

	export let width: 'basic' | 'full-width' = 'basic';

	const pollForUser = async (token: string) => {
		const apiUser = await cloud.login.user.get(token).catch(() => null);
		if (apiUser) {
			$user = apiUser;
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
			.then(cloud.login.token.create)
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

{#if $user}
	<Button kind="plain" color="destructive" on:click={() => ($user = null)}>Log out</Button>
{:else if $token !== null}
	{#await Promise.all([open($token.url), pollForUser($token.token)])}
		<div>Log in in your system browser</div>
	{/await}
	<p>
		<button class="underline" on:click={() => open($authUrl)}>Click here</button>
		if you are not redirected automatically, you can
	</p>
{:else}
	<Button {width} loading={signUpOrLoginLoading} color="purple" on:click={onSignUpOrLoginClick}>
		Sign up or Log in
	</Button>
{/if}
