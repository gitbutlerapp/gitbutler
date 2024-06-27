<script lang="ts">
	import Link from '../shared/Link.svelte';
	import Button from '$lib/shared/Button.svelte';
	import { UserService, type LoginToken } from '$lib/stores/user';
	import { getContext } from '$lib/utils/context';

	const userService = getContext(UserService);
	const loading = userService.loading;
	const user = userService.user;

	export let wide = false;

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
	<p class="helper-text text-base-body-12">
		Your browser should have been opened. Please log into your GitButler account there.
		{#if token}
			If you were not redirected automatically, open <Link
				target="_blank"
				rel="noreferrer"
				href={token.url}
			>
				this link
			</Link>
		{/if}
	</p>
{:else}
	<div>
		<Button
			style="pop"
			kind="solid"
			loading={$loading}
			icon="signin"
			{wide}
			on:mousedown={async () => {
				await userService.login();
			}}
		>
			Sign up or Log in
		</Button>
	</div>
{/if}

<style>
	.helper-text {
		color: var(--clr-text-2);
	}
</style>
