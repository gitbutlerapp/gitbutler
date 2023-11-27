<script lang="ts">
	import { getCloudApiClient, type LoginToken, type User } from '$lib/backend/cloud';
	import * as toasts from '$lib/utils/toasts';
	import { open } from '@tauri-apps/api/shell';
	import Button from './Button.svelte';
	import type { UserService } from '$lib/stores/user';
	import { derived, writable } from 'svelte/store';

	const cloud = getCloudApiClient();

	export let user: User | undefined;
	export let userService: UserService;

	const pollForUser = async (token: string) => {
		const apiUser = await cloud.login.user.get(token).catch(() => null);
		if (apiUser) {
			userService.set(apiUser);
			return;
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
</script>

{#if user}
	<Button
		kind="filled"
		color="error"
		on:click={async () => {
			user = undefined;
			await userService.logout();
		}}>Log out</Button
	>
{:else if $token}
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
	<Button loading={signUpOrLoginLoading} color="primary" on:click={onSignUpOrLoginClick}>
		Sign up or Log in
	</Button>
{/if}
