import { apiToUserSimple, type ApiUserSimple, type UserSimple } from '$lib/users/types';
import type { LoadableData } from '$lib/network/types';
import type { BrandedId } from '$lib/utils/branding';

export type ApiDiffPatch = {
	type: 'context' | 'added' | 'removed';
	left?: number;
	right?: number;
	line: string;
};

export type ApiChatMessage = {
	comment: string;
	mentions: ApiUserSimple[];
	diff_patch_array: ApiDiffPatch[] | null;
	diff_path: string | null;
	diff_sha: string | null;
	issue: boolean;
	outdated: boolean;
	resolved: boolean;
	uuid: string;
	created_at: string;
	updated_at: string;
	thread_id: string | null;
	user: ApiUserSimple;
};

export type DiffPatch = {
	type: 'context' | 'added' | 'removed';
	left?: number;
	right?: number;
	line: string;
};

export type ChatMessage = {
	comment: string;
	mentions: UserSimple[];
	diffPatchArray: DiffPatch[] | undefined;
	diffPath: string | undefined;
	diffSha: string | undefined;
	issue: boolean;
	outdated: boolean;
	resolved: boolean;
	uuid: string;
	createdAt: string;
	updatedAt: string;
	threadId: string | undefined;
	user: UserSimple;
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

export function apiToChatMessage(apiChatMessage: ApiChatMessage): ChatMessage {
	return {
		comment: apiChatMessage.comment,
		mentions: apiChatMessage.mentions.map(apiToUserSimple),
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
		user: apiToUserSimple(apiChatMessage.user)
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
	 * Displayable text for the chat message.
	 * This is used to display the chat message in notification without having to de-embed the mentions.
	 */
	displayable_text?: string;
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
	 * Displayable text for the chat message.
	 * This is used to display the chat message in notification without having to de-embed the mentions.
	 */
	displayableText?: string;
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
		displayable_text: params.displayableText,
		change_id: params.changeId,
		thread_id: params.threadId,
		issue: params.issue,
		diff_path: params.diffPath,
		diff_sha: params.diffSha,
		range: params.range
	};
}

export type ApiPatchChatMessageParams = {
	chat_uuid: string;
	/**
	 * Chat message issue is resolved
	 */
	resolved: boolean;
};

export type PatchChatMessageParams = {
	projectId: string;
	changeId?: string;
	messageUuid: string;
	/**
	 * Chat message issue is resolved
	 */
	resolved: boolean;
};

export function toApiPatchChatMessageParams(
	params: PatchChatMessageParams
): ApiPatchChatMessageParams {
	return {
		chat_uuid: params.messageUuid,
		resolved: params.resolved
	};
}
