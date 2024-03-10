<script lang="ts">
	import Button from './Button.svelte';
	import { getCloudApiClient, type LoginToken } from '$lib/backend/cloud';
	import * as toasts from '$lib/utils/toasts';
	import { openExternalUrl } from '$lib/utils/url';
	import { createEventDispatcher } from 'svelte';
	import { derived, writable } from 'svelte/store';
	import type { UserService } from '$lib/stores/user';

	const cloud = getCloudApiClient();

	export let userService: UserService;
	export let minimal = false;
	export let wide = false;

	$: user$ = userService.user$;

	const token = writable<LoginToken | null>(null);
	const authUrl = derived(token, ($token) => $token?.url as string);

	let signUpOrLoginLoading = false;

	async function pollForUser(token: string) {
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
	}

	async function onSignUpOrLoginClick() {
		signUpOrLoginLoading = true;
		try {
			token.set(await cloud.login.token.create());
		} catch (err: any) {
			console.error(err);
			toasts.error('Could not create login token');
		} finally {
			signUpOrLoginLoading = false;
			dispatch('login');
		}
	}

	// create on:login event and on:logout event
	const dispatch = createEventDispatcher<{ login: void; logout: void }>();
</script>

{#if $user$}
	<Button
		kind="filled"
		color="error"
		{wide}
		icon="signout"
		on:click={async () => {
			await userService.logout();
			dispatch('logout');
		}}>Log out</Button
	>
{:else if $token}
	{#if minimal}
		Your browser should have been opened. Please log into your GitButler account there.
	{:else}
		{#await Promise.all([openExternalUrl($token.url), pollForUser($token.token)])}
			<div class="text-light-700">
				Your browser should have been opened. Please log into your GitButler account there.
			</div>
		{/await}
		<p>
			If you were not redirected automatically, you can
			<button class="underline" on:click={() => openExternalUrl($authUrl)}>click here</button>
		</p>
	{/if}
{:else}
	<div>
		<Button
			loading={signUpOrLoginLoading}
			color="primary"
			icon="signin"
			on:mousedown={onSignUpOrLoginClick}
			{wide}
		>
			Sign up or Log in
		</Button>
	</div>
{/if}
