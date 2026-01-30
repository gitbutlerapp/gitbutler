<script lang="ts">
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import { InfoMessage, TestId } from '@gitbutler/ui';
	import { gravatarUrlFromEmail } from '@gitbutler/ui/components/avatar/gravatar';
	import type { Toast } from '$lib/notifications/toasts';

	type Props = {
		toast: Toast;
		dismiss: () => void;
	};

	const { toast, dismiss }: Props = $props();

	const userService = inject(USER_SERVICE);
	const incomingUser = $derived(userService.incomingUserLogin);
	const incomingUserEmail = $derived($incomingUser?.email ?? '');
	const incomingUserName = $derived(
		$incomingUser?.login ?? $incomingUser?.name ?? incomingUserEmail ?? 'unknown'
	);
	const incomingUserAvatarUrl = $derived($incomingUser?.picture);

	async function getUserAvatarURL(): Promise<string | null> {
		if (incomingUserAvatarUrl) return incomingUserAvatarUrl;
		if (incomingUserEmail) {
			const gravatarUrl = await gravatarUrlFromEmail(incomingUserEmail);
			return gravatarUrl;
		}
		return null;
	}

	function rejectLogin() {
		userService.rejectIncomingUser();
		dismiss();
	}
</script>

<InfoMessage
	testId={TestId.ToastLoginConfirmation}
	style="info"
	error={toast.error}
	primaryLabel="Accept login"
	primaryAction={() => {
		if ($incomingUser) {
			userService.acceptIncomingUser($incomingUser);
		}
		dismiss();
	}}
	secondaryLabel="Reject login"
	secondaryAction={rejectLogin}
	shadow
>
	{#snippet title()}
		Confirm the login of {incomingUserName}
	{/snippet}

	{#snippet customContent()}
		{#await getUserAvatarURL() then url}
			<div class="login-confirmation__profile-pic">
				<img src={url} alt="User Avatar" width="64" height="64" style="border-radius: 50%;" />
			</div>
		{/await}
	{/snippet}

	{#snippet content()}
		<p>
			A new login attempt has been detected for the user
			<strong>{incomingUserName}</strong>.
			<br />
			Would you like to accept this login?
		</p>
	{/snippet}
</InfoMessage>

<style>
	.login-confirmation__profile-pic {
		display: flex;
		justify-content: center;
		margin-bottom: 12px;
	}
</style>
