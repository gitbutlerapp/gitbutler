<script lang="ts">
	import WelcomeAction from '$components/WelcomeAction.svelte';
	import signinSvg from '$lib/assets/signin.svg?raw';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import { Button, LinkButton } from '@gitbutler/ui';

	import { writable } from 'svelte/store';

	const {
		dimMessage,
		prompt = 'Enable features like auto branch and commit message generation.'
	}: {
		dimMessage?: boolean;
		prompt?: string;
	} = $props();

	const aborted = writable(false);

	const userService = inject(USER_SERVICE);
	const loading = userService.loading;
	const user = userService.user;
</script>

{#if !$user}
	<WelcomeAction
		title="Log in or Sign up"
		loading={$loading}
		onclick={async () => {
			$aborted = false;
			await userService.login(aborted);
		}}
		rowReverse
		{dimMessage}
	>
		{#snippet icon()}
			{@html signinSvg}
		{/snippet}
		{#snippet message()}
			{prompt}
			For manual login, copy the
			<LinkButton
				icon="copy-small"
				onclick={async () => {
					$aborted = false;
					await userService.loginAndCopyLink(aborted);
				}}
			>
				the login link
			</LinkButton>
		{/snippet}
	</WelcomeAction>

	{#if $loading}
		<div>
			<Button kind="outline" onclick={() => ($aborted = true)} loading={$aborted}
				>Cancel login attempt</Button
			>
		</div>
	{/if}
{/if}
