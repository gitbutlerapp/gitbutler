<script lang="ts">
	import { PatchService } from '@gitbutler/shared/branches/patchService';
	import { ChatChannelsService } from '@gitbutler/shared/chat/chatChannelsService';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import DropDownButton from '@gitbutler/ui/DropDownButton.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';

	interface Props {
		projectId: string;
		branchId: string;
		branchUuid: string;
		changeId: string;
	}

	let { branchUuid, projectId, branchId, changeId }: Props = $props();

	const patchService = getContext(PatchService);
	const chatChannelService = getContext(ChatChannelsService);

	let message = $state<string>();
	let isSendingMessage = $state(false);
	let isExecuting = $state(false);

	async function sendMessage(message: string | undefined, issue?: boolean) {
		if (message === undefined || message.trim() === '') {
			return;
		}

		await chatChannelService.sendChatMessage({
			projectId,
			branchId,
			changeId,
			chat: message,
			issue
		});
	}

	async function handleSendMessage(issue?: boolean) {
		if (isSendingMessage) return;
		isSendingMessage = true;
		try {
			await sendMessage(message, issue);
		} finally {
			message = undefined;
			isSendingMessage = false;
		}
	}

	async function handleKeyDown(
		event: KeyboardEvent & { currentTarget: EventTarget & HTMLTextAreaElement }
	) {
		message = event.currentTarget.value;
		if (event.key === 'Enter' && !event.shiftKey) {
			event.preventDefault();
			event.stopPropagation();
			await handleSendMessage();
			return;
		}
	}

	async function handleClickSend() {
		await handleSendMessage();
	}

	const actionLabels = {
		approve: 'Approve commit',
		requestChanges: 'Request changes'
	} as const;

	type Action = keyof typeof actionLabels;

	let action = $state<Action>('approve');
	let dropDownButton = $state<ReturnType<typeof DropDownButton>>();

	async function approve() {
		await patchService.updatePatch(branchUuid, changeId, { signOff: true, message });
		message = undefined;
	}

	async function requestChanges() {
		await patchService.updatePatch(branchUuid, changeId, { signOff: false });
	}

	async function handleActionClick() {
		if (isExecuting) return;
		isExecuting = true;
		try {
			switch (action) {
				case 'approve':
					await approve();
					break;
				case 'requestChanges':
					await handleSendMessage(true);
					await requestChanges();
					break;
			}
		} finally {
			isExecuting = false;
		}
	}

	const actionButtonLabel = $derived.by(() => {
		const suffix = message ? ' & Comment' : '';
		return actionLabels[action] + suffix;
	});
</script>

<div class="chat-input">
	<div class="chat-input__content-container">
		<Textarea bind:value={message} unstyled autofocus onkeydown={handleKeyDown} />
		<div class="chat-input__actions">
			<div class="chat-input__secondary-actions">
				<p>📋</p>
				<p>😈</p>
			</div>
			<div class="chat-input__action-buttons">
				<DropDownButton
					bind:this={dropDownButton}
					loading={isSendingMessage || isExecuting}
					style="neutral"
					kind="outline"
					onclick={handleActionClick}
				>
					{actionButtonLabel}
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
				<Button
					style="pop"
					loading={isSendingMessage || isExecuting}
					disabled={!message}
					onclick={handleClickSend}>Comment</Button
				>
			</div>
		</div>
	</div>
</div>

<style lang="postcss">
	.chat-input {
		flex-shrink: 0;
		display: flex;
		justify-content: space-between;
		padding: 16px;
		border-top: 1px solid #ccc;
	}

	.chat-input__content-container {
		flex-grow: 1;
		display: flex;
		flex-direction: column;
		padding: 12px;

		border-radius: var(--m, 6px);
		border: 1px solid var(--border-2, #d4d0ce);
	}

	.chat-input__actions {
		flex-grow: 1;
		display: flex;
		justify-content: space-between;
	}

	.chat-input__secondary-actions {
		display: flex;

		p {
			padding: 6px;
			opacity: 0.5;
		}
	}

	.chat-input__action-buttons {
		display: flex;
		gap: 4px;
	}
</style>
