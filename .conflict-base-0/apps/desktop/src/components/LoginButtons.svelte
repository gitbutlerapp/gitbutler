<script lang="ts">
	import { OnboardingEvent, POSTHOG_WRAPPER } from '$lib/analytics/posthog';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import { Button } from '@gitbutler/ui';
	import { writable } from 'svelte/store';

	const userService = inject(USER_SERVICE);
	const loading = userService.loading;
	const user = userService.user;
	const posthog = inject(POSTHOG_WRAPPER);

	const aborted = writable(false);

	let showAbort = $state(false);
	let abortTimer: ReturnType<typeof setTimeout> | null = null;

	// Watch for loading state changes
	$effect(() => {
		if ($loading) {
			// Start timer when loading begins
			abortTimer = setTimeout(() => {
				showAbort = true;
			}, 7000);
		} else {
			// Clear timer and hide abort button when loading stops
			if (abortTimer) {
				clearTimeout(abortTimer);
				abortTimer = null;
			}
			showAbort = false;
		}

		// Cleanup function
		return () => {
			if (abortTimer) {
				clearTimeout(abortTimer);
				abortTimer = null;
			}
		};
	});
</script>

{#if !$user}
	{#if !showAbort}
		<Button
			style="pop"
			loading={$loading}
			icon="signin"
			onclick={async () => {
				$aborted = false;
				posthog.captureOnboarding(OnboardingEvent.LoginGitButler);
				await userService.login(aborted);
			}}
		>
			Sign up or Log in
		</Button>
	{:else}
		<Button
			kind="outline"
			onclick={() => {
				$aborted = true;
				posthog.captureOnboarding(OnboardingEvent.CancelLoginGitButler);
			}}
			loading={$aborted}
		>
			Cancel login
		</Button>
	{/if}
{/if}
