<script lang="ts">
	import Button from './Button.svelte';
	import { getCloudApiClient, type LoginToken } from '$lib/backend/cloud';
	import * as toasts from '$lib/utils/toasts';
	import { open } from '@tauri-apps/api/shell';
	import { createEventDispatcher } from 'svelte';
	import { derived, writable } from 'svelte/store';
	import type { UserService } from '$lib/stores/user';

	const cloud = getCloudApiClient();

	export let userService: UserService;
	export let minimal = false;
	export let wide = false;

	$: user$ = userService.user$;

	const pollForUser = async (token: string) => {
		const apiUser = await cloud.login.user.get(token).catch(() => null);
		if (apiUser) {
			userService.setUser(apiUser);
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
			.finally(() => {
				signUpOrLoginLoading = false;
				onLogin();
			});
	};
	const token = writable<LoginToken | null>(null);
	const authUrl = derived(token, ($token) => $token?.url as string);

	// create on:login event and on:logout event
	const dispatch = createEventDispatcher<{ login: void; logout: void }>();

	const onLogin = () => {
		dispatch('login');
	};

	const onLogout = () => {
		dispatch('logout');
	};
</script>

{#if $user$}
	<Button
		kind="filled"
		color="error"
		{wide}
		icon="signout"
		on:click={async () => {
			await userService.logout();
			onLogout();
		}}>Log out</Button
	>
{:else if $token}
	{#if minimal}
		Your browser should have been opened. Please log into your GitButler account there.
	{:else}
		{#await Promise.all([open($token.url), pollForUser($token.token)])}
			<div class="text-light-700">
				Your browser should have been opened. Please log into your GitButler account there.
			</div>
		{/await}
		<p>
			If you were not redirected automatically, you can
			<button class="underline" on:click={() => open($authUrl)}>click here</button>
		</p>
	{/if}
{:else}
	<div>
		<Button
			loading={signUpOrLoginLoading}
			color="primary"
			icon="signin"
			on:click={onSignUpOrLoginClick}
			{wide}
		>
			Sign up or Log in
		</Button>
	</div>
{/if}
