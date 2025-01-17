<script lang="ts">
	import Button from '@gitbutler/ui/Button.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';

	interface Props {
		sendMessage: (message: string | undefined) => Promise<void>;
	}

	let { sendMessage }: Props = $props();

	let message = $state<string>();
	let isSendingMessage = $state(false);

	const disableIssueButton = true; // TODO: $derived(!message || isSendingMessage);

	async function handleSendMessage() {
		if (isSendingMessage) return;
		isSendingMessage = true;
		try {
			await sendMessage(message);
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

	async function handleClick() {
		await handleSendMessage();
	}
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
				<Button disabled={disableIssueButton}>Mark as an issue</Button>
				<Button loading={isSendingMessage} disabled={!message} onclick={handleClick}>Send</Button>
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
