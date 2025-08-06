import { invoke } from '$lib/backend/ipc';
import { hasTauriExtra } from '$lib/state/backendQuery';
import { providesList, ReduxTag } from '$lib/state/tags';
import { type TranscriptEntry, type ClaudeCodeEvent } from '$lib/vibeCenter/types';
import { InjectionToken } from '@gitbutler/shared/context';
import type { ClientState } from '$lib/state/clientState.svelte';

export const CLAUDE_CODE_SERVICE = new InjectionToken<ClaudeCodeService>('Claude code service');

export class ClaudeCodeService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(clientState: ClientState['backendApi']) {
		this.api = injectEndpoints(clientState);
	}

	get sendMessage() {
		return this.api.endpoints.sendMessage.useMutation;
	}

	get transcript() {
		return this.api.endpoints.getTranscript.useQuery;
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
			getTranscript: build.query<TranscriptEntry[], { projectId: string; stackId: string }>({
				extraOptions: { command: 'claude_get_transcript' },
				query: (args) => args,
				providesTags: [providesList(ReduxTag.EditChangesSinceInitial)],
				async onCacheEntryAdded(arg, lifecycleApi) {
					if (!hasTauriExtra(lifecycleApi.extra)) {
						throw new Error('Redux dependency Tauri not found!');
					}
					const { listen } = lifecycleApi.extra.tauri;
					await lifecycleApi.cacheDataLoaded;
					const unsubscribe = listen<ClaudeCodeEvent>(
						`project://${arg.projectId}/claude/${arg.stackId}/message_recieved`,
						async (_event) => {
							const transcript = await invoke<TranscriptEntry[]>('claude_get_transcript', arg);
							lifecycleApi.updateCachedData(() => transcript);
						}
					);
					await lifecycleApi.cacheEntryRemoved;
					unsubscribe();
				}
			})
		})
	});
}
