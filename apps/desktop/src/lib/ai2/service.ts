import { invalidatesList, providesList, ReduxTag } from '$lib/state/tags';
import type { ClientState } from '$lib/state/clientState.svelte';

export type MessageRole = 'user' | 'assistant' | 'system' | 'tool';

export type Message = {
	role: MessageRole;
	content: string;
	tool_call_id?: string;
};

export class Ai2Service {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(state: ClientState) {
		this.api = injectEndpoints(state.backendApi);
	}

	get isOpenRouterTokenSet() {
		return this.api.endpoints.agentIsOpenRouterTokenSet.useQuery();
	}

	get setOpenRouterToken() {
		return this.api.endpoints.agentSetOpenRouterToken.useMutation();
	}

	conversations({ projectId }: { projectId: string }) {
		return this.api.endpoints.agentListAllConversations.useQuery({ projectId });
	}

	get createConversation() {
		return this.api.endpoints.agentCreateConversation.useMutation();
	}

	get sendMessage() {
		return this.api.endpoints.agentSendMessage.useMutation();
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			agentListAllConversations: build.query<{ [key: string]: Message[] }, { projectId: string }>({
				query: ({ projectId }) => ({
					command: 'agent_list_all_conversations',
					params: { projectId }
				}),
				providesTags: [providesList(ReduxTag.AiConversations)]
			}),
			agentIsOpenRouterTokenSet: build.query<boolean, void>({
				query: () => ({
					command: 'agent_is_open_router_token_set',
					params: {}
				}),
				providesTags: [providesList(ReduxTag.OpenRouterToken)]
			}),
			agentSetOpenRouterToken: build.mutation<void, { token: string | undefined }>({
				query: ({ token }) => ({
					command: 'agent_set_open_router_token',
					params: { token }
				}),
				invalidatesTags: [invalidatesList(ReduxTag.OpenRouterToken)]
			}),
			agentCreateConversation: build.mutation<string, { projectId: string }>({
				query: ({ projectId }) => ({
					command: 'agent_create_conversation',
					params: { projectId }
				}),
				invalidatesTags: [invalidatesList(ReduxTag.AiConversations)]
			}),
			agentSendMessage: build.mutation<
				string,
				{ projectId: string; conversationId: string; message: string }
			>({
				query: ({ projectId, conversationId, message }) => ({
					command: 'agent_send_message',
					params: { projectId, conversationId, message }
				}),
				invalidatesTags: [invalidatesList(ReduxTag.AiConversations)]
			})
		})
	});
}
