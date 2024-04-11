<script lang="ts">
	import Button from './Button.svelte';
	import { UserService, type LoginToken } from '$lib/stores/user';
	import { getContext } from '$lib/utils/context';
	import * as toasts from '$lib/utils/toasts';
	import { openExternalUrl } from '$lib/utils/url';

	const userService = getContext(UserService);
	const user = userService.user;

	export let minimal = false;
	export let wide = false;

	let signUpOrLoginLoading = false;
	let token: LoginToken | undefined;
</script>

{#if $user}
	<Button
		style="error"
		kind="solid"
		{wide}
		icon="signout"
		on:click={async () => {
			await userService.logout();
		}}>Log out</Button
	>
{:else if token}
	{#if minimal}
		Your browser should have been opened. Please log into your GitButler account there.
	{:else}
		<div class="text-light-700">
			Your browser should have been opened. Please log into your GitButler account there.
		</div>
		<p>
			If you were not redirected automatically, you can
			<button class="underline" on:click={() => token && openExternalUrl(token.url)}>
				Click here
			</button>
		</p>
	{/if}
{:else}
	<div>
		<Button
			style="pop"
			kind="solid"
			loading={signUpOrLoginLoading}
			icon="signin"
			{wide}
			on:mousedown={async () => {
				signUpOrLoginLoading = true;
				try {
					token = await userService.createLoginToken();
					await userService.login(token);
				} catch (err) {
					console.error(err);
					toasts.error('Could not create login token');
				} finally {
					signUpOrLoginLoading = false;
				}
			}}
		>
			Sign up or Log in
		</Button>
	</div>
{/if}
