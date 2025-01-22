<script lang="ts">
	import ChatInputProps from '$lib/components/chat/ChatInput.svelte';
	import Message from '$lib/components/chat/Message.svelte';
	import blankChat from '$lib/images/blank-chat.svg?raw';
	import { getChatChannel } from '@gitbutler/shared/chat/chatChannelsPreview.svelte';
	import { ChatChannelsService } from '@gitbutler/shared/chat/chatChannelsService';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';

	interface Props {
		projectId: string;
		branchId: string;
		changeId: string;
	}

	const { projectId, changeId, branchId }: Props = $props();

	const appState = getContext(AppState);
	const chatChannelService = getContext(ChatChannelsService);
	const chatChannel = getChatChannel(appState, chatChannelService, projectId, changeId);

	async function sendMessage(message: string | undefined) {
		if (message === undefined || message.trim() === '') {
			return;
		}

		await chatChannelService.sendChatMessage({
			projectId,
			branchId,
			changeId,
			chat: message
		});
	}
</script>

<div class="chat-card">
	<div class="chat-messages">
		{#if chatChannel}
			<Loading loadable={chatChannel.current}>
				{#snippet children(channel)}
					{#if channel.messages.length > 0}
						{#each channel.messages as message}
							<Message {message} />
						{/each}
					{:else}
						<div class="blank-state">
							<div class="blank-state-content">
								{@html blankChat}
								<div class="blank-message">
									<div class="blank-message-title">Give some feedback!</div>
									<p class="blank-message-text">
										If you're here, you must be important. This patch can use your help. Leave a
										comment or ask a question. Does this look right to you? How can it be improved?
										Is it perfect? Just let us know!
									</p>
								</div>
							</div>
						</div>
					{/if}
				{/snippet}
			</Loading>
		{/if}
	</div>
	<ChatInputProps {sendMessage} />
</div>

<style lang="postcss">
	.chat-card {
		width: 100%;
		height: 50vh;
		overflow: hidden;
		display: flex;
		flex-direction: column;
		height: 100%;

		border-radius: var(--ml, 10px);
		border: 1px solid var(--border-2, #d4d0ce);
		background: var(--bg-1, #fff);
	}

	.chat-messages {
		flex-grow: 1;
		overflow-y: scroll;
		scrollbar-width: none;
		&::-webkit-scrollbar {
			display: none;
		}
	}

	.blank-state {
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: flex-start;
		padding-left: 30px;
		margin-top: 20px;
	}

	.blank-state-content {
		display: flex;
		flex-direction: column;
		align-items: left;
		gap: 1rem;
		text-align: left;
		padding-left: 40px;
	}

	.blank-message {
		padding-left: 17px;
	}

	.blank-message-title {
		font-size: 1.3rem;
		font-weight: 600;
		color: var(--text-2, #333);
		margin-top: 10px;
	}

	.blank-message-text {
		font-size: 0.9rem;
		color: var(--text-2, #777);
		margin-top: 10px;
		width: 80%;
	}
</style>
