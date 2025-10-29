<script lang="ts">
	import AttachmentList from '$components/codegen/AttachmentList.svelte';
	import { messageQueueSelectors, messageQueueSlice } from '$lib/codegen/messageQueueSlice';
	import { CLIENT_STATE } from '$lib/state/clientState.svelte';
	import { inject } from '@gitbutler/core/context';
	import { Icon, Button } from '@gitbutler/ui';
	import { slide } from 'svelte/transition';
	import type { PromptAttachment } from '$lib/codegen/types';

	type Message = {
		thinkingLevel: any;
		model: any;
		permissionMode: any;
		prompt: string;
		attachments?: PromptAttachment[];
	};

	interface Props {
		projectId: string;
		stackId: string;
		branchName: string;
	}

	const { projectId, stackId, branchName }: Props = $props();

	const clientState = inject(CLIENT_STATE);

	const queue = $derived(
		messageQueueSelectors
			.selectAll(clientState.messageQueue)
			.find((q) => q.head === branchName && q.stackId === stackId && q.projectId === projectId)
	);

	function deleteMessage(message: any) {
		if (queue) {
			clientState.dispatch(
				messageQueueSlice.actions.upsert({
					...queue,
					messages: queue.messages.filter((m) => m !== message)
				})
			);
		}
	}

	function deleteAllMessages() {
		if (queue) {
			clientState.dispatch(
				messageQueueSlice.actions.upsert({
					...queue,
					messages: []
				})
			);
		}
	}
</script>

{#snippet messageContent(message: Message)}
	<div class="message-content">
		<p class="text-13 text-body message-text">{message.prompt}</p>

		{#if message.attachments && message.attachments.length > 0}
			<AttachmentList attachments={message.attachments} showRemoveButton={false} />
		{/if}
	</div>

	<div class="message-actions">
		<Button
			kind="ghost"
			icon="bin"
			size="tag"
			shrinkable
			onclick={() => deleteMessage(message)}
			tooltip="Remove from queue"
		/>
	</div>
{/snippet}

{#if queue && queue.messages.length > 0}
	<div class="queued-messages" in:slide={{ duration: 300 }}>
		<div class="queue-header">
			<div class="flex items-center gap-8">
				<Icon name="time" />
				<span class="text-12">Queued messages</span>
			</div>

			{#if queue.messages.length > 1}
				<button
					type="button"
					onclick={deleteAllMessages}
					class="text-11 text-semibold queue-clear-button">Clear all</button
				>
			{/if}
		</div>

		<div class="messages-list">
			{#each queue.messages as message, i (`${message.prompt}-${message.thinkingLevel}-${message.model}-${i}`)}
				<div class="message-item">
					{@render messageContent(message)}
				</div>
			{/each}
		</div>
	</div>
{/if}

<style lang="postcss">
	.queued-messages {
		padding: 12px;
		overflow: hidden;
		border-bottom: 1px solid var(--clr-border-3);
		background-color: var(--clr-bg-2);
	}

	.queue-header {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 10px;
		gap: 8px;
		color: var(--clr-text-2);
	}

	.queue-clear-button {
		color: var(--clr-text-2);
		transition: color var(--transition-fast);

		&:hover {
			color: var(--clr-text-1);
		}
	}

	.messages-list {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: 8px;
	}

	.message-item {
		display: flex;
		position: relative;
		max-width: 500px;
		padding-left: 30px;
		padding: 10px 14px;
		overflow: hidden;
		gap: 8px;
		border: 1px dashed var(--clr-border-1);
		border-radius: var(--radius-ml);
		border-bottom-right-radius: 0;

		&:hover .message-actions {
			transform: scale(1);
			opacity: 1;
		}
	}

	.message-content {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: 10px;
	}

	.message-text {
		display: -webkit-box;
		overflow: hidden;
		word-break: break-word;
		-webkit-line-clamp: 2;
		-webkit-box-orient: vertical;
		text-overflow: ellipsis;
	}

	.message-actions {
		display: flex;
		position: absolute;
		right: 4px;
		bottom: 4px;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		padding: 2px;
		gap: 2px;
		transform: scale(0.9);
		transform-origin: bottom right;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
		opacity: 0;
		transition:
			opacity var(--transition-fast),
			transform var(--transition-medium);
	}
</style>
