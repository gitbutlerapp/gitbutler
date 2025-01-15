<script lang="ts">
	import ChatInputProps from '$lib/components/chat/ChatInputProps.svelte';
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

	let message = $state<string>();

	async function sendMessage() {
		if (message === undefined || message.trim() === '') {
			return;
		}

		await chatChannelService.sendChatMessage({
			projectId,
			branchId,
			changeId,
			chat: message
		});

		message = undefined;
	}
</script>

<div class="chat-card">
	{#if chatChannel}
		<Loading loadable={chatChannel.current}>
			{#snippet children(channel)}
				<div class="chat-messages">
					{#each channel.messages as message}
						<!-- TODO: Actually retrieve the correct data -->
						<Message author={message.userId.toString()} content={JSON.stringify(message.comment)} />
					{/each}
				</div>
			{/snippet}
		</Loading>
	{/if}
	<ChatInputProps bind:message {sendMessage} />
</div>

<style lang="postcss">
	.chat-card {
		width: 100%;
		height: 50vh;
		display: flex;
		flex-direction: column;
		height: 100%;
		border: 1px solid #ccc;
	}

	.chat-messages {
		flex-grow: 1;
	}
</style>
