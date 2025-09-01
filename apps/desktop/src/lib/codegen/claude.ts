import {
	type ClaudeMessage,
	type ClaudePermissionRequest,
	type ClaudeSessionDetails,
	type ThinkingLevel
} from '$lib/codegen/types';
import { hasBackendExtra } from '$lib/state/backendQuery';
import { invalidatesItem, providesItem, ReduxTag } from '$lib/state/tags';
import { InjectionToken } from '@gitbutler/core/context';
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

	get permissionRequests() {
		return this.api.endpoints.getPermissionRequests.useQuery;
	}

	get updatePermissionRequest() {
		return this.api.endpoints.updatePermissionRequest.mutate;
	}

	get cancelSession() {
		return this.api.endpoints.cancelSession.mutate;
	}

	get checkAvailable() {
		return this.api.endpoints.checkAvailable.useQuery;
	}

	get fetchCheckAvailable() {
		return this.api.endpoints.checkAvailable.fetch;
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
					thinkingLevel: ThinkingLevel;
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
					if (!hasBackendExtra(lifecycleApi.extra)) {
						throw new Error('Redux dependency Backend not found!');
					}
					const { listen } = lifecycleApi.extra.backend;
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
			}),
			getPermissionRequests: build.query<ClaudePermissionRequest[], { projectId: string }>({
				extraOptions: { command: 'claude_list_permission_requests' },
				query: (args) => args,
				providesTags: (_result, _error, args) => [
					...providesItem(ReduxTag.ClaudePermissionRequests, args.projectId)
				],
				async onCacheEntryAdded(arg, lifecycleApi) {
					if (!hasBackendExtra(lifecycleApi.extra)) {
						throw new Error('Redux dependency Backend not found!');
					}
					const { listen, invoke } = lifecycleApi.extra.backend;
					await lifecycleApi.cacheDataLoaded;
					const unsubscribe = listen<unknown>(
						`project://${arg.projectId}/claude-permission-requests`,
						async (_) => {
							const value = await invoke<ClaudePermissionRequest[]>(
								'claude_list_permission_requests',
								{ projectId: arg.projectId }
							);
							lifecycleApi.updateCachedData(() => value);
						}
					);
					await lifecycleApi.cacheEntryRemoved;
					unsubscribe();
				}
			}),
			updatePermissionRequest: build.mutation<
				undefined,
				{
					projectId: string;
					requestId: string;
					approval: boolean;
				}
			>({
				extraOptions: {
					command: 'claude_update_permission_request',
					actionName: 'Update Permission Request'
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.ClaudePermissionRequests, args.projectId)
				]
			}),
			cancelSession: build.mutation<
				boolean,
				{
					projectId: string;
					stackId: string;
				}
			>({
				extraOptions: {
					command: 'claude_cancel_session',
					actionName: 'Cancel Session'
				},
				query: (args) => args
			}),
			checkAvailable: build.query<boolean, undefined>({
				extraOptions: { command: 'claude_check_available' },
				query: (args) => args,
				// For some unholy reason, this is represented in seconds. This
				// can be a little slow, and the value is unlikely to change so,
				// let's cache it for a long time.
				keepUnusedDataFor: 60 * 60 * 24
			})
		})
	});
}
