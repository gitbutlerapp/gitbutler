import { chatChannelsSelectors } from './chatChannelsSlice';
import { createChannelKey, type LoadableChatChannel } from '$lib/chat/types';
import { registerInterest, type InView } from '$lib/interest/registerInterestFunction.svelte';
import { map } from '$lib/network/loadable';
import { deduplicateBy } from '$lib/utils/array';
import type { ChatChannelsService } from '$lib/chat/chatChannelsService';
import type { AppChatChannelsState } from '$lib/redux/store.svelte';
import type { Reactive } from '$lib/storeUtils';
import type { UserSimple } from '$lib/users/types';

export function getChatChannel(
	appState: AppChatChannelsState,
	chatMessagesService: ChatChannelsService,
	projectId: string,
	changeId: string,
	inView?: InView
): Reactive<LoadableChatChannel | undefined> {
	const chatMessagesInterest = chatMessagesService.getChatChannelInterest(projectId, changeId);
	registerInterest(chatMessagesInterest, inView);

	const chatChannelKey = createChannelKey(projectId, changeId);
	const chatChannel = $derived(
		chatChannelsSelectors.selectById(appState.chatChannels, chatChannelKey)
	);

	return {
		get current() {
			return chatChannel;
		}
	};
}

export function getChatChannelParticipants(
	appState: AppChatChannelsState,
	chatMessagesService: ChatChannelsService,
	projectId: string,
	changeId: string,
	inView?: InView
): Reactive<UserSimple[] | undefined> {
	const chatMessagesInterest = chatMessagesService.getChatChannelInterest(projectId, changeId);
	registerInterest(chatMessagesInterest, inView);

	const chatChannelKey = createChannelKey(projectId, changeId);
	const chatChannelParticipants = $derived.by(() => {
		const channel = chatChannelsSelectors.selectById(appState.chatChannels, chatChannelKey);
		return map(channel, (channel) =>
			deduplicateBy(
				channel.messages.map((message) => message.user),
				'id'
			)
		);
	});

	return {
		get current() {
			return chatChannelParticipants;
		}
	};
}
