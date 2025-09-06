<script lang="ts">
	import { USER_SERVICE, type LoginToken } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import { Button, Link } from '@gitbutler/ui';
	import { writable } from 'svelte/store';

	const userService = inject(USER_SERVICE);
	const loading = userService.loading;
	const user = userService.user;

	interface Props {
		wide?: boolean;
	}

	const { wide = false }: Props = $props();

	const aborted = writable(false);

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
	<div class="login-buttons">
		<Button
			style="pop"
			loading={$loading}
			icon="signin"
			{wide}
			onclick={async () => {
				$aborted = false;
				await userService.login(aborted);
			}}
		>
			Sign up or Log in
		</Button>

		{#if $loading}
			<div>
				<Button kind="outline" onclick={() => ($aborted = true)} loading={$aborted}
					>Cancel login attempt</Button
				>
			</div>
		{/if}
	</div>
{/if}

<style>
	.helper-text {
		color: var(--clr-text-2);
	}

	.login-buttons {
		display: flex;
		gap: 8px;
	}
</style>
