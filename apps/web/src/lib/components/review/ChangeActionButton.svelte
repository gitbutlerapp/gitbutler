<script lang="ts">
	import LoginModal from '$lib/components/LoginModal.svelte';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import { PatchService } from '@gitbutler/shared/patches/patchService';
	import { type Patch } from '@gitbutler/shared/patches/types';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import DropDownButton from '@gitbutler/ui/DropDownButton.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';

	interface Props {
		branchUuid: string;
		patch: Patch;
		isUserLoggedIn: boolean;
	}

	const actionLabels = {
		approve: 'Approve commit',
		requestChanges: 'Request changes'
	} as const;

	type Action = keyof typeof actionLabels;
	type UserActionType = 'requested-changes' | 'approved' | 'not-reviewed';

	const { patch, branchUuid, isUserLoggedIn }: Props = $props();

	const patchService = getContext(PatchService);
	const userService = getContext(UserService);
	const user = userService.user;

	const userAction = $derived.by<UserActionType>(() => {
		if (!$user) return 'not-reviewed';
		if (patch.reviewAll.rejected.some((rejector) => rejector.id === $user.id)) {
			return 'requested-changes';
		} else if (patch.reviewAll.signedOff.some((approver) => approver.id === $user.id)) {
			return 'approved';
		}

		return 'not-reviewed';
	});

	let loginModal = $state<LoginModal>();
	let action = $state<Action>('approve');
	let isExecuting = $state(false);
	let dropDownButton = $state<ReturnType<typeof DropDownButton>>();

	const buttonColor = $derived.by(() => {
		switch (action) {
			case 'approve':
				return 'pop';
			case 'requestChanges':
				return 'warning';
		}
	});

	const icon = $derived.by(() => {
		switch (action) {
			case 'approve':
				return 'success';
			case 'requestChanges':
				return 'refresh-in-circle';
		}
	});

	async function approve() {
		await patchService.updatePatch(branchUuid, patch.changeId, { signOff: true });
	}

	async function requestChanges() {
		await patchService.updatePatch(branchUuid, patch.changeId, { signOff: false });
	}

	async function handleClick() {
		if (!isUserLoggedIn) {
			loginModal?.show();
			return;
		}

		if (isExecuting) return;
		isExecuting = true;

		try {
			switch (action) {
				case 'approve':
					await approve();
					break;
				case 'requestChanges':
					await requestChanges();
					break;
			}
		} finally {
			isExecuting = false;
		}
	}

	function handleRequestChanges() {
		action = 'requestChanges';
		handleClick();
	}

	function handleApprove() {
		action = 'approve';
		handleClick();
	}
</script>

{#if userAction === 'approved'}
	<div class="my-status-wrap approved">
		<div class="text-12 my-status approved">
			<span>You approved this</span><Icon name="tick-small" />
		</div>

		<button class="text-12 change-status-btn" type="button" onclick={handleRequestChanges}>
			<span>Change status</span>
			<Icon name="refresh-small" />
		</button>
	</div>
{:else if userAction === 'requested-changes'}
	<div class="my-status-wrap">
		<div class="text-12 my-status requested-changes">
			<span>You requested changes</span><Icon name="refresh-small" />
		</div>

		<button class="text-12 change-status-btn" type="button" onclick={handleApprove}>
			<span>Change status</span>
			<Icon name="tick-small" />
		</button>
	</div>
{:else}
	<DropDownButton
		bind:this={dropDownButton}
		loading={isExecuting}
		menuPosition="top"
		{icon}
		style={buttonColor}
		onclick={handleClick}
	>
		{actionLabels[action]}
		{#snippet contextMenuSlot()}
			<ContextMenuSection>
				<ContextMenuItem
					label={actionLabels.approve}
					onclick={() => {
						action = 'approve';
						dropDownButton?.close();
					}}
				/>
				<ContextMenuItem
					label={actionLabels.requestChanges}
					onclick={() => {
						action = 'requestChanges';
						dropDownButton?.close();
					}}
				/>
			</ContextMenuSection>
		{/snippet}
	</DropDownButton>
{/if}

<LoginModal bind:this={loginModal}>
	To approve this commit or request changes, you need to be logged in.
</LoginModal>

<style lang="postcss">
	.my-status-wrap {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.my-status {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 4px 8px;
		height: 100%;
		border-radius: var(--radius-m);

		&.approved {
			background-color: var(--clr-theme-succ-soft);
			color: var(--clr-theme-succ-on-soft);
		}

		&.requested-changes {
			background-color: var(--clr-theme-warn-soft);
			color: var(--clr-theme-warn-on-soft);
		}
	}

	.change-status-btn {
		display: flex;
		align-items: center;
		gap: 4px;
		color: var(--clr-text-2);

		& span {
			text-decoration: underline;
			text-decoration-style: dotted;
			text-underline-offset: 3px;
		}
	}
</style>
