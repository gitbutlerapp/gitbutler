import { InjectionToken } from '@gitbutler/core/context';
import type { IBackend } from '$lib/backend';
import type { BackendApi } from '$lib/state/clientState.svelte';

export const BUTBOT_SERVICE = new InjectionToken<ButbotService>('ButbotService');

type ChatMessage = {
	type: 'user';
	content: string;
};

export type ForgeReviewFilter = 'today' | 'thisWeek' | 'thisMonth' | 'all';

export class ButbotService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		api: BackendApi,
		private backend: IBackend
	) {
		this.api = injectEndpoints(api);
	}

	listenForTokens(projectId: string, messageId: string, listener: (token: string) => void) {
		const unlisten = this.backend.listen(`project://${projectId}/token-updates`, (event) => {
			if (!isTokenUpdateEventPayload(event.payload)) {
				return;
			}
			if (event.payload.messageId === messageId) {
				listener(event.payload.token);
			}
		});

		return () => {
			unlisten();
		};
	}

	get forgeBranchChat() {
		return this.api.endpoints.forgeBranchChat.useMutation();
	}
}

function injectEndpoints(api: BackendApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			forgeBranchChat: build.mutation<
				string,
				{
					projectId: string;
					branch: string;
					messageId: string;
					chatMessages: ChatMessage[];
					filter: ForgeReviewFilter | null;
					model: string;
				}
			>({
				extraOptions: {
					command: 'forge_branch_chat'
				},
				query: (args) => args
			})
		})
	});
}

function isTokenUpdateEventPayload(
	payload: unknown
): payload is { messageId: string; token: string } {
	return (
		typeof payload === 'object' &&
		payload !== null &&
		'messageId' in payload &&
		typeof (payload as any).messageId === 'string' &&
		'token' in payload &&
		typeof (payload as any).token === 'string'
	);
}
