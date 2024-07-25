<script lang="ts">
	import WelcomeAction from './WelcomeAction.svelte';
	import signinSvg from '$lib/assets/no-projects/signin.svg?raw';
	import { UserService } from '$lib/stores/user';
	import { getContext } from '$lib/utils/context';

	export let prompt: string =
		'Enable GitButler features like automatic branch and commit message generation.';

	const userService = getContext(UserService);
	const loading = userService.loading;
	const user = userService.user;
</script>

{#if !$user}
	<WelcomeAction
		title="Log in or Sign up"
		loading={$loading}
		on:mousedown={async () => {
			await userService.login();
		}}
	>
		<svelte:fragment slot="icon">
			{@html signinSvg}
		</svelte:fragment>
		<svelte:fragment slot="message">{prompt}</svelte:fragment>
	</WelcomeAction>
{/if}
