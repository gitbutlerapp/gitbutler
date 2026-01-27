<script lang="ts">
	import { OnboardingEvent, POSTHOG_WRAPPER } from '$lib/analytics/posthog';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import { Button } from '@gitbutler/ui';

	const userService = inject(USER_SERVICE);
	const posthog = inject(POSTHOG_WRAPPER);
</script>

<Button
	style="pop"
	icon="signin"
	onclick={async () => {
		posthog.captureOnboarding(OnboardingEvent.LoginGitButler);
		await userService.openLoginPage();
	}}
>
	Sign up or Log in
</Button>
