import { chatChannelTable } from '$lib/chat/chatChannelsSlice';
import {
	apiToChatMessage,
	createChannelKey,
	toApiCreateChatMessageParams,
	toApiPatchChatMessageParams,
	type ApiChatMessage,
	type LoadableChatChannel,
	type PatchChatMessageParams,
	type SendChatMessageParams
} from '$lib/chat/types';
import { InterestStore, type Interest } from '$lib/interest/interestStore';
import { errorToLoadable } from '$lib/network/loadable';
import { POLLING_GLACIALLY } from '$lib/polling';
import { InjectionToken } from '@gitbutler/core/context';
import type { HttpClient } from '$lib/network/httpClient';
import type { AppDispatch } from '$lib/redux/store.svelte';

export const CHAT_CHANNELS_SERVICE: InjectionToken<ChatChannelsService> = new InjectionToken(
	'ChatChannelsService'
);

export class ChatChannelsService {
	private readonly chatMessagesInterests = new InterestStore<{
		projectId: string;
		changeId?: string;
	}>(POLLING_GLACIALLY);

	constructor(
		private readonly httpClient: HttpClient,
		private readonly appDispatch: AppDispatch
	) {}

	getChatChannelInterest(projectId: string, changeId?: string): Interest {
		return this.chatMessagesInterests
			.findOrCreateSubscribable({ projectId, changeId }, async () => {
				const chatChannelKey = createChannelKey(projectId, changeId);
				this.appDispatch.dispatch(
					chatChannelTable.addOne({ status: 'loading', id: chatChannelKey })
				);
				try {
					const apiChatMessages = await this.httpClient.get<ApiChatMessage[]>(
						`chat_messages/${projectId}/chats/${changeId ?? ''}`
					);

					// Return the messages in reverse order so that
					// the newest messages are at the bottom
					apiChatMessages.reverse();

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

					this.appDispatch.dispatch(chatChannelTable.upsertOne(chatChannel));
				} catch (error: unknown) {
					this.appDispatch.dispatch(
						chatChannelTable.addOne(errorToLoadable(error, chatChannelKey))
					);
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

	async patchChatMessage(params: PatchChatMessageParams): Promise<void> {
		try {
			await this.httpClient.patch(`chat_messages/${params.projectId}/chat/${params.messageUuid}`, {
				body: toApiPatchChatMessageParams(params)
			});

			// Re-fetch the chat messages to get the new message
			this.chatMessagesInterests.invalidate({
				projectId: params.projectId,
				changeId: params.changeId
			});
		} catch (error) {
			console.error('Failed to update chat message', error);
		}
	}
}
