<script lang="ts">
	import { USER_SERVICE } from "$lib/user/userService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Button, ModalHeader, ModalFooter, SkeletonBone } from "@gitbutler/ui";
	import { gravatarUrlFromEmail } from "@gitbutler/ui/components/avatar/gravatar";
	import type { LoginConfirmationModalState } from "$lib/state/uiState.svelte";

	type Props = {
		data: LoginConfirmationModalState;
		close: () => void;
	};

	const { close }: Props = $props();

	const userService = inject(USER_SERVICE);
	const incomingUserEmail = $derived(userService.incomingUserLogin?.email);
	const incomingUserName = $derived(
		userService.incomingUserLogin?.login ??
			userService.incomingUserLogin?.name ??
			incomingUserEmail ??
			"unknown",
	);
	const incomingUserAvatarUrl = $derived(userService.incomingUserLogin?.picture);

	async function getUserAvatarURL(): Promise<string | null> {
		if (incomingUserAvatarUrl) return incomingUserAvatarUrl;
		if (incomingUserEmail) {
			const gravatarUrl = await gravatarUrlFromEmail(incomingUserEmail);
			return gravatarUrl;
		}
		return null;
	}

	function acceptLogin() {
		if (userService.incomingUserLogin) {
			userService.acceptIncomingUser(userService.incomingUserLogin);
		}
		close();
	}

	function rejectLogin() {
		userService.rejectIncomingUser();
		close();
	}

	const avatarSize = "3.25rem";
</script>

<ModalHeader type="info">Confirm login attempt for {incomingUserName}</ModalHeader>
<div class="modal-content">
	{#await getUserAvatarURL()}
		<SkeletonBone width={avatarSize} height={avatarSize} radius="100%" />
	{:then url}
		<img src={url} alt="" class="login-avatar" style:height={avatarSize} style:width={avatarSize} />
	{/await}

	<p class="text-13 text-body clr-text-2">
		A new login attempt has been detected for the user with email
		<span class="text-bold clr-text-1">{incomingUserEmail ?? "-unknown-"}</span>. Would you like to
		accept this login?
	</p>
</div>
<ModalFooter>
	<Button kind="outline" onclick={rejectLogin}>Reject</Button>
	<Button style="pop" onclick={acceptLogin}>Accept login</Button>
</ModalFooter>

<style lang="postcss">
	.modal-content {
		display: flex;
		padding: 0 16px 16px 16px;
		gap: 16px;
	}

	.login-avatar {
		border-radius: 50%;
		background-color: var(--bg-2);
	}
</style>
