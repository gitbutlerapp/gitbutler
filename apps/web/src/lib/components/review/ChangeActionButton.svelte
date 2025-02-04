<script lang="ts">
	import { PatchService } from '@gitbutler/shared/branches/patchService';
	import { getContext } from '@gitbutler/shared/context';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import DropDownButton from '@gitbutler/ui/DropDownButton.svelte';
	import type { Patch } from '@gitbutler/shared/branches/types';

	interface Props {
		branchUuid: string;
		patch: Patch;
	}

	const actionLabels = {
		approve: 'Approve commit',
		requestChanges: 'Request changes'
	} as const;

	type Action = keyof typeof actionLabels;

	const { patch, branchUuid }: Props = $props();

	const patchService = getContext(PatchService);

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
</script>

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
