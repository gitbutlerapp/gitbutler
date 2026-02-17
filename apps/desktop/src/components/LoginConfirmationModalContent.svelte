<script lang="ts">
	import { USER_SERVICE } from "$lib/user/userService";
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
	const incomingUser = $derived(userService.incomingUserLogin);
	const incomingUserEmail = $derived($incomingUser?.email ?? "");
	const incomingUserName = $derived(
		$incomingUser?.login ?? $incomingUser?.name ?? incomingUserEmail ?? "unknown",
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

	function acceptLogin() {
		if ($incomingUser) {
			userService.acceptIncomingUser($incomingUser);
		}
		close();
	}

	function rejectLogin() {
		userService.rejectIncomingUser();
		close();
	}

	const avatarSize = "3.25rem";
</script>

<ModalHeader type="info">Confirm login attempt</ModalHeader>
<div class="modal-content">
	{#await getUserAvatarURL()}
		<SkeletonBone width={avatarSize} height={avatarSize} radius="100%" />
	{:then url}
		<img src={url} alt="" class="login-avatar" style:height={avatarSize} style:width={avatarSize} />
	{/await}

	<p class="text-13 text-body clr-text-2">
		A new login attempt has been detected for the user
		<span class="text-bold clr-text-1">{incomingUserName}</span>. Would you like to accept this
		login?
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
		background-color: var(--clr-bg-2);
	}
</style>
