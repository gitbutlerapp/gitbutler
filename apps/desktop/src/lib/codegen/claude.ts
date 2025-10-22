import {
	type ClaudeCheckResult,
	type ClaudeMessage,
	type ClaudePermissionRequest,
	type ClaudeSessionDetails,
	type ThinkingLevel,
	type ModelType,
	type PermissionMode,
	type PromptTemplate,
	type PromptDir,
	type McpConfig,
	type SubAgent,
	type AttachmentInput
} from '$lib/codegen/types';
import { hasBackendExtra } from '$lib/state/backendQuery';
import {
	invalidatesItem,
	invalidatesList,
	providesItem,
	providesList,
	ReduxTag
} from '$lib/state/tags';
import { InjectionToken } from '@gitbutler/core/context';
import type { ClientState } from '$lib/state/clientState.svelte';

export const CLAUDE_CODE_SERVICE = new InjectionToken<ClaudeCodeService>('Claude code service');

export class ClaudeCodeService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(clientState: ClientState['backendApi']) {
		this.api = injectEndpoints(clientState);
	}

	get sendMessageMutate() {
		return this.api.endpoints.sendMessage.mutate;
	}

	get sendMessage() {
		return this.api.endpoints.sendMessage.useMutation();
	}

	get messages() {
		return this.api.endpoints.getMessages.useQuery;
	}

	get fetchMessages() {
		return this.api.endpoints.getMessages.fetch;
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

	isStackActive(projectId: string, stackId: string) {
		return this.api.endpoints.isStackActive.useQuery({
			projectId,
			stackId
		});
	}

	get fetchIsStackActive() {
		return this.api.endpoints.isStackActive.fetch;
	}

	sessionDetails(projectId: string, sessionId: string) {
		return this.api.endpoints.getSessionDetails.useQuery({
			projectId,
			sessionId
		});
	}

	get fetchSessionDetails() {
		return this.api.endpoints.getSessionDetails.fetch;
	}

	promptTemplates(projectId: string) {
		return this.api.endpoints.listPromptTemplates.useQuery({ projectId });
	}

	get fetchPromptTemplates() {
		return this.api.endpoints.listPromptTemplates.fetch;
	}

	promptDirs(projectId: string) {
		return this.api.endpoints.getPromptDirs.useQuery({ projectId });
	}

	get fetchPromptDirs() {
		return this.api.endpoints.getPromptDirs.fetch;
	}

	get createPromptDir() {
		return this.api.endpoints.createPromptDir.mutate;
	}

	get mcpConfig() {
		return this.api.endpoints.getMcpConfig.useQuery;
	}

	get subAgents() {
		return this.api.endpoints.getSubAgents.useQuery;
	}

	get verifyPath() {
		return this.api.endpoints.verifyPath.mutate;
	}

	get compactHistory() {
		return this.api.endpoints.compactHistory.mutate;
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
					model: ModelType;
					permissionMode: PermissionMode;
					disabledMcpServers: string[];
					addDirs: string[];
					attachments?: AttachmentInput[];
				}
			>({
				extraOptions: {
					command: 'claude_send_message',
					actionName: 'Send message'
				},
				query: (args) => args,
				invalidatesTags: [invalidatesList(ReduxTag.ClaudeStackActive)],
				async onCacheEntryAdded(arg, lifecycleApi) {
					api.util.invalidateTags([invalidatesList(ReduxTag.ClaudeStackActive)]);
					await lifecycleApi.cacheDataLoaded;
					api.util.invalidateTags([invalidatesList(ReduxTag.ClaudeStackActive)]);
				}
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
					api.util.invalidateTags([invalidatesList(ReduxTag.ClaudeStackActive)]);
					await lifecycleApi.cacheDataLoaded;

					const unsubscribe = listen<ClaudeMessage>(
						`project://${arg.projectId}/claude/${arg.stackId}/message_recieved`,
						async (event) => {
							const message = event.payload;
							lifecycleApi.updateCachedData((events) => {
								events.push(message);
							});

							api.util.invalidateTags([invalidatesList(ReduxTag.ClaudeStackActive)]);
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
				query: (args) => args,
				invalidatesTags: [invalidatesList(ReduxTag.ClaudeStackActive)]
			}),
			checkAvailable: build.query<ClaudeCheckResult, undefined>({
				extraOptions: { command: 'claude_check_available' },
				query: (args) => args,
				// For some unholy reason, this is represented in seconds. This
				// can be a little slow, and the value is unlikely to change so,
				// let's cache it for a long time.
				keepUnusedDataFor: 60 * 60 * 24
			}),
			isStackActive: build.query<boolean, { projectId: string; stackId: string }>({
				extraOptions: { command: 'claude_is_stack_active' },
				query: (args) => args,
				providesTags: [providesList(ReduxTag.ClaudeStackActive)],
				async onCacheEntryAdded(arg, lifecycleApi) {
					if (!hasBackendExtra(lifecycleApi.extra)) {
						throw new Error('Redux dependency Backend not found!');
					}
					const { listen, invoke } = lifecycleApi.extra.backend;
					api.util.invalidateTags([invalidatesList(ReduxTag.ClaudeStackActive)]);
					await lifecycleApi.cacheDataLoaded;

					const unsubscribe = listen<ClaudeMessage>(
						`project://${arg.projectId}/claude/${arg.stackId}/message_recieved`,
						async () => {
							const active = await invoke<boolean>('claude_is_stack_active', arg);
							lifecycleApi.updateCachedData(() => active);
						}
					);
					await lifecycleApi.cacheEntryRemoved;
					unsubscribe();
				}
			}),
			listPromptTemplates: build.query<PromptTemplate[], { projectId: string }>({
				extraOptions: { command: 'claude_list_prompt_templates' },
				query: (args) => args
			}),
			getPromptDirs: build.query<PromptDir[], { projectId: string }>({
				extraOptions: { command: 'claude_get_prompt_dirs' },
				query: (args) => args
			}),
			createPromptDir: build.mutation<undefined, { projectId: string; path: string }>({
				extraOptions: {
					command: 'claude_maybe_create_prompt_dir',
					actionName: 'Create Prompt Directory'
				},
				query: (args) => args
			}),
			getMcpConfig: build.query<McpConfig, { projectId: string }>({
				extraOptions: { command: 'claude_get_mcp_config' },
				query: (args) => args
			}),
			getSubAgents: build.query<SubAgent[], { projectId: string }>({
				extraOptions: { command: 'claude_get_sub_agents' },
				query: (args) => args
			}),
			verifyPath: build.mutation<
				boolean,
				{
					projectId: string;
					path: string;
				}
			>({
				extraOptions: {
					command: 'claude_verify_path',
					actionName: 'Verify Path'
				},
				query: (args) => args
			}),
			compactHistory: build.mutation<
				undefined,
				{
					projectId: string;
					stackId: string;
				}
			>({
				extraOptions: {
					command: 'claude_compact_history',
					actionName: 'Compact History'
				},
				query: (args) => args,
				invalidatesTags: [invalidatesList(ReduxTag.ClaudeStackActive)]
			})
		})
	});
}
