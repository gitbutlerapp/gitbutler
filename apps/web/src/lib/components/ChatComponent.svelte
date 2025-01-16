<script lang="ts">
	import ChatInputProps from '$lib/components/chat/ChatInput.svelte';
	import Message from '$lib/components/chat/Message.svelte';
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
					{#each channel.messages as message}
						<Message {message} />
					{/each}
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
	}
</style>
