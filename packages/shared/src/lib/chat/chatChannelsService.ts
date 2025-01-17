import { upsertChatChannel } from './chatChannelsSlice';
import {
	apiToChatMessage,
	createChannelKey,
	toApiCreateChatMessageParams,
	type ApiChatMessage,
	type LoadableChatChannel,
	type SendChatMessageParams
} from '$lib/chat/types';
import { InterestStore, type Interest } from '$lib/interest/interestStore';
import { errorToLoadable } from '$lib/network/loadable';
import { POLLING_REGULAR } from '$lib/polling';
import type { HttpClient } from '$lib/network/httpClient';
import type { AppDispatch } from '$lib/redux/store.svelte';

export class ChatChannelsService {
	private readonly chatMessagesInterests = new InterestStore<{
		projectId: string;
		changeId?: string;
	}>(POLLING_REGULAR);

	constructor(
		private readonly httpClient: HttpClient,
		private readonly appDispatch: AppDispatch
	) {}

	getChatChannelInterest(projectId: string, changeId?: string): Interest {
		return this.chatMessagesInterests
			.findOrCreateSubscribable({ projectId, changeId }, async () => {
				const chatChannelKey = createChannelKey(projectId, changeId);
				try {
					const apiChatMessages = await this.httpClient.get<ApiChatMessage[]>(
						`chat_messages/${projectId}/chats/${changeId ?? ''}`
					);

					const chatChannel: LoadableChatChannel = {
						status: 'found',
						id: chatChannelKey,
						value: {
							id: chatChannelKey,
							projectId,
							changeId,
							messages: apiChatMessages.map(apiToChatMessage)
						}
					};

					this.appDispatch.dispatch(upsertChatChannel(chatChannel));
				} catch (error: unknown) {
					this.appDispatch.dispatch(upsertChatChannel(errorToLoadable(error, chatChannelKey)));
				}
			})
			.createInterest();
	}

	async refetchChatChannel(projectId: string, changeId?: string): Promise<void> {
		await this.chatMessagesInterests.invalidate({ projectId, changeId });
	}

	async sendChatMessage(params: SendChatMessageParams): Promise<void> {
		try {
			await this.httpClient.post(`chat_messages/${params.projectId}/branch/${params.branchId}`, {
				body: toApiCreateChatMessageParams(params)
			});

			// Re-fetch the chat messages to get the new message
			this.chatMessagesInterests.invalidate({
				projectId: params.projectId,
				changeId: params.changeId
			});
		} catch (error) {
			console.error('Failed to send chat message', error);
		}
	}
}
