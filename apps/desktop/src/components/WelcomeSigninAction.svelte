<script lang="ts">
	import WelcomeAction from './WelcomeAction.svelte';
	import signinSvg from '$lib/assets/signin.svg?raw';
	import { UserService } from '$lib/stores/user';
	import { getContext } from '@gitbutler/shared/context';
	import LinkButton from '@gitbutler/ui/LinkButton.svelte';

	const {
		dimMessage,
		prompt = 'Enable features like auto branch and commit message generation.'
	}: {
		dimMessage?: boolean;
		prompt?: string;
	} = $props();

	const userService = getContext(UserService);
	const loading = userService.loading;
	const user = userService.user;
</script>

{#if !$user}
	<WelcomeAction
		title="Log in or Sign up"
		loading={$loading}
		onclick={async () => {
			await userService.login();
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
					await userService.loginAndCopyLink();
				}}
			>
				the login link
			</LinkButton>
		{/snippet}
	</WelcomeAction>
{/if}
