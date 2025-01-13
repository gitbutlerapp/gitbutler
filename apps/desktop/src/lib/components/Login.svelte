<script lang="ts">
	import Link from '../shared/Link.svelte';
	import { UserService, type LoginToken } from '$lib/stores/user';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';

	const userService = getContext(UserService);
	const loading = userService.loading;
	const user = userService.user;

	interface Props {
		wide?: boolean;
	}

	const { wide = false }: Props = $props();

	let token: LoginToken | undefined;
</script>

{#if $user}
	<Button
		style="error"
		{wide}
		icon="signout"
		onclick={async () => {
			await userService.logout();
		}}>Log out</Button
	>
{:else if token}
	<p class="helper-text text-12 text-body">
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
			loading={$loading}
			icon="signin"
			{wide}
			onclick={async () => {
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
