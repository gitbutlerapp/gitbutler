<script lang="ts">
	import WelcomeAction from './WelcomeAction.svelte';
	import ImgThemed from '$lib/components/ImgThemed.svelte';
	import { UserService } from '$lib/stores/user';
	import { getContextByClass } from '$lib/utils/context';

	const userService = getContextByClass(UserService);
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
			<ImgThemed
				imgSet={{
					light: '/images/welcome-signin-light.webp',
					dark: '/images/welcome-signin-dark.webp'
				}}
			/>
		</svelte:fragment>
		<svelte:fragment slot="message">
			Enable GitButler features like automatic branch and commit message generation.
		</svelte:fragment>
	</WelcomeAction>
{/if}
