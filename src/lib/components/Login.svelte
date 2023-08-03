<script lang="ts">
	import { getCloudApiClient, type LoginToken } from '$lib/api/cloud/api';
	import { toasts } from '$lib';
	import { userStore } from '$lib/stores/user';
	import { derived, writable } from '@square/svelte-store';
	import { open } from '@tauri-apps/api/shell';
	import Button from './Button';
	import { goto } from '$app/navigation';

	const cloud = getCloudApiClient();
	const user = userStore;

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
				console.error(e);
				toasts.error('Something went wrong');
			})
			.finally(() => (signUpOrLoginLoading = false));
	};
	const token = writable<LoginToken | null>(null);
	const authUrl = derived(token, ($token) => $token?.url as string);

	$: if ($user) {
		goto('/user/');
	}
</script>

{#if $user}
	<Button kind="plain" color="destructive" on:click={() => ($user = null)}>Log out</Button>
{:else if $token !== null}
	{#await Promise.all([open($token.url), pollForUser($token.token)])}
		<div class="text-light-700">
			Your browser should have been opened. Please log into your GitButler account there.
		</div>
	{/await}
	<p>
		If you were not redirected automatically, you can
		<button class="underline" on:click={() => open($authUrl)}>click here</button>
	</p>
{:else}
	<Button {width} loading={signUpOrLoginLoading} color="purple" on:click={onSignUpOrLoginClick}>
		Sign up or Log in
	</Button>
{/if}
