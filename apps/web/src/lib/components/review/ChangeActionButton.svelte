<script lang="ts">
	import { UserService } from '$lib/user/userService';
	import { PatchService } from '@gitbutler/shared/branches/patchService';
	import { type Patch } from '@gitbutler/shared/branches/types';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import DropDownButton from '@gitbutler/ui/DropDownButton.svelte';

	interface Props {
		branchUuid: string;
		patch: Patch;
	}

	const actionLabels = {
		approve: 'Approve commit',
		requestChanges: 'Request changes'
	} as const;

	type Action = keyof typeof actionLabels;
	type UserActionType = 'requested-changes' | 'approved' | 'not-reviewed';

	const { patch, branchUuid }: Props = $props();

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

	$effect(() => {
		console.log('userAction', userAction);
	});
</script>

{#if userAction === 'approved'}
	<div class="my-status-wrap">
		<div class="user-status-label approved">
			<span class="text-12">You approved this</span>
		</div>
		<Button
			loading={isExecuting}
			icon="undo-small"
			style="warning"
			class="my-status-btn"
			onclick={handleRequestChanges}
		>
			Revert approval
		</Button>
	</div>
{:else if userAction === 'requested-changes'}
	<div class="my-status-wrap">
		<div class="user-status-label requested-changes">
			<span class="text-12">You requested changes</span>
		</div>
		<Button
			loading={isExecuting}
			icon="tick-small"
			style="success"
			class="my-status-btn"
			onclick={handleApprove}
		>
			Approve
		</Button>
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

<style lang="postcss">
	.my-status-wrap {
		display: flex;
		align-items: center;
	}

	.user-status-label {
		user-select: none;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 0 12px;
		height: 100%;
		font-style: italic;
		border-radius: var(--radius-m) 0 0 var(--radius-m);

		color: var(--clr-text-2);

		&.approved {
			background-color: var(--clr-theme-succ-soft);
			color: var(--clr-theme-succ-on-soft);
		}

		&.requested-changes {
			background-color: var(--clr-theme-warn-soft);
			color: var(--clr-theme-warn-on-soft);
		}
	}

	:global(.my-status-wrap .my-status-btn) {
		border-radius: 0 var(--radius-m) var(--radius-m) 0;
	}
</style>
