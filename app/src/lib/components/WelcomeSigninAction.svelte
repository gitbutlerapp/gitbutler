<script lang="ts">
	import WelcomeAction from './WelcomeAction.svelte';
	import signinSvg from '$lib/assets/no-projects/signin.svg?raw';
	import { UserService } from '$lib/stores/user';
	import { getContext } from '$lib/utils/context';

	export let prompt: string =
		'Enable GitButler features like automatic branch and commit message generation.';

	const userService = getContext(UserService);
	const user = userService.user;

	let loginSignupLoading = false;

	async function onLoginOrSignup() {
		loginSignupLoading = true;
		try {
			await userService.login();
		} catch {
			loginSignupLoading = false;
		}
	}

	// reset loading state after 60 seconds
	// this is to prevent the loading state from getting stuck
	// if the user closes the tab before the request is finished
	setTimeout(() => {
		loginSignupLoading = false;
	}, 60 * 1000);
</script>

{#if !$user}
	<WelcomeAction
		title="Log in or Sign up"
		loading={loginSignupLoading}
		on:mousedown={onLoginOrSignup}
	>
		<svelte:fragment slot="icon">
			{@html signinSvg}
		</svelte:fragment>
		<svelte:fragment slot="message">{prompt}</svelte:fragment>
	</WelcomeAction>
{/if}
