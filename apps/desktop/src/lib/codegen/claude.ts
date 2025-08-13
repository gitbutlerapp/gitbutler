import { type ClaudeMessage, type ClaudeSessionDetails } from '$lib/codegen/types';
import { hasTauriExtra } from '$lib/state/backendQuery';
import { providesItem, ReduxTag } from '$lib/state/tags';
import { InjectionToken } from '@gitbutler/shared/context';
import type { ClientState } from '$lib/state/clientState.svelte';

export const CLAUDE_CODE_SERVICE = new InjectionToken<ClaudeCodeService>('Claude code service');

export class ClaudeCodeService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(clientState: ClientState['backendApi']) {
		this.api = injectEndpoints(clientState);
	}

	get sendMessage() {
		return this.api.endpoints.sendMessage.mutate;
	}

	get messages() {
		return this.api.endpoints.getMessages.useQuery;
	}

	sessionDetails(projectId: string, sessionId: string) {
		return this.api.endpoints.getSessionDetails.useQuery({
			projectId,
			sessionId
		});
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			sendMessage: build.mutation<
				undefined,
				{
					projectId: string;
					stackId: string;
					message: string;
				}
			>({
				extraOptions: {
					command: 'claude_send_message',
					actionName: 'Send message'
				},
				query: (args) => args
			}),
			getSessionDetails: build.query<
				ClaudeSessionDetails,
				{ projectId: string; sessionId: string }
			>({
				extraOptions: { command: 'claude_get_session_details' },
				query: (args) => args,
				providesTags: (_result, _error, args) => [
					...providesItem(ReduxTag.ClaudeSessionDetails, args.projectId + args.sessionId)
				]
			}),
			getMessages: build.query<ClaudeMessage[], { projectId: string; stackId: string }>({
				extraOptions: { command: 'claude_get_messages' },
				query: (args) => args,
				providesTags: (_result, _error, args) => [
					...providesItem(ReduxTag.ClaudeCodeTranscript, args.projectId + args.stackId)
				],
				async onCacheEntryAdded(arg, lifecycleApi) {
					if (!hasTauriExtra(lifecycleApi.extra)) {
						throw new Error('Redux dependency Tauri not found!');
					}
					const { listen } = lifecycleApi.extra.tauri;
					await lifecycleApi.cacheDataLoaded;
					const unsubscribe = listen<ClaudeMessage>(
						`project://${arg.projectId}/claude/${arg.stackId}/message_recieved`,
						async (event) => {
							lifecycleApi.updateCachedData((events) => {
								events.push(event.payload);
							});
						}
					);
					await lifecycleApi.cacheEntryRemoved;
					unsubscribe();
				}
			})
		})
	});
}
