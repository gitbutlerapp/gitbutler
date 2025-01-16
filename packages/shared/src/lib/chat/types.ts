import type { LoadableData } from '$lib/network/types';
import type { BrandedId } from '$lib/utils/branding';

export type ApiChatMessageUser = {
	id: number;
	avatar_url: string | null;
	email: string | null;
	login: string | null;
	name: string | null;
};

export type ApiChatMessage = {
	comment: string;
	diff_patch_array: string[] | null;
	diff_path: string | null;
	diff_sha: string | null;
	issue: boolean;
	outdated: boolean;
	resolved: boolean;
	uuid: string;
	created_at: string;
	updated_at: string;
	thread_id: string | null;
	user: ApiChatMessageUser;
};

export type ChatMessageUser = {
	id: number;
	avatarUrl: string | undefined;
	email: string | undefined;
	login: string | undefined;
	name: string | undefined;
};

export type ChatMessage = {
	comment: string;
	diffPatchArray: string[] | undefined;
	diffPath: string | undefined;
	diffSha: string | undefined;
	issue: boolean;
	outdated: boolean;
	resolved: boolean;
	uuid: string;
	createdAt: string;
	updatedAt: string;
	threadId: string | undefined;
	user: ChatMessageUser;
};

type ChatChannelId = BrandedId<'ChatChannelId'>;

export type ChatChannel = {
	/**
	 * The unique identifier of the chat channel.
	 *
	 * Built from the project ID and the change ID.
	 */
	id: ChatChannelId;
	projectId: string;
	changeId?: string;
	messages: ChatMessage[];
};

export function createChannelKey(projectId: string, changeId: string | undefined): ChatChannelId {
	if (changeId === undefined) {
		return projectId as ChatChannelId;
	}
	return `${projectId}:${changeId}` as ChatChannelId;
}

export type LoadableChatChannel = LoadableData<ChatChannel, ChatChannel['id']>;

function apiToChatMessageUser(apiChatMessageUser: ApiChatMessageUser): ChatMessageUser {
	return {
		id: apiChatMessageUser.id,
		avatarUrl: apiChatMessageUser.avatar_url ?? undefined,
		email: apiChatMessageUser.email ?? undefined,
		login: apiChatMessageUser.login ?? undefined,
		name: apiChatMessageUser.name ?? undefined
	};
}

export function apiToChatMessage(apiChatMessage: ApiChatMessage): ChatMessage {
	return {
		comment: apiChatMessage.comment,
		diffPatchArray: apiChatMessage.diff_patch_array ?? undefined,
		diffPath: apiChatMessage.diff_path ?? undefined,
		diffSha: apiChatMessage.diff_sha ?? undefined,
		issue: apiChatMessage.issue,
		outdated: apiChatMessage.outdated,
		resolved: apiChatMessage.resolved,
		uuid: apiChatMessage.uuid,
		createdAt: apiChatMessage.created_at,
		updatedAt: apiChatMessage.updated_at,
		threadId: apiChatMessage.thread_id ?? undefined,
		user: apiToChatMessageUser(apiChatMessage.user)
	};
}

export type ApiCreateChatMessageParams = {
	/**
	 * Branch ID
	 */
	branch_id: string;
	/**
	 * Chat message
	 */
	chat: string;
	/**
	 * Change ID
	 */
	change_id?: string;
	/**
	 * UUID of chat thread
	 */
	thread_id?: string;
	/**
	 * This comment is an issue
	 * @default false
	 */
	issue?: boolean;
	/**
	 * Path of patch file to comment on
	 */
	diff_path?: string;
	/**
	 * SHA of Diff to comment on
	 */
	diff_sha?: string;
	/**
	 * Range of Diff to comment on (ex: L15 or L15-R50)
	 */
	range?: string;
};

export type SendChatMessageParams = {
	projectId: string;
	branchId: string;
	/**
	 * Chat message
	 */
	chat: string;
	/**
	 * Commit Change ID
	 */
	changeId?: string;
	threadId?: string;
	/**
	 * This comment is an issue
	 * @default false
	 */
	issue?: boolean;
	/**
	 * Path of patch file to comment on
	 */
	diffPath?: string;
	/**
	 * SHA of Diff to comment on
	 */
	diffSha?: string;
	/**
	 * Range of Diff to comment on (ex: L15 or L15-R50)
	 */
	range?: string;
};

export function toApiCreateChatMessageParams(
	params: SendChatMessageParams
): ApiCreateChatMessageParams {
	return {
		branch_id: params.branchId,
		chat: params.chat,
		change_id: params.changeId,
		thread_id: params.threadId,
		issue: params.issue,
		diff_path: params.diffPath,
		diff_sha: params.diffSha,
		range: params.range
	};
}
