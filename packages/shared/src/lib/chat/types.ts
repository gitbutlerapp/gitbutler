import { apiToUserSimple, type ApiUserSimple, type UserSimple } from '$lib/users/types';
import type { LoadableData } from '$lib/network/types';
import type { BrandedId } from '$lib/utils/branding';

export type ApiDiffPatch = {
	type: 'context' | 'added' | 'removed';
	left?: number;
	right?: number;
	line: string;
};

export type ApiChatMessageReaction = {
	reaction: string;
	users: ApiUserSimple[];
};

export type ApiChatMessageInReplyTo = {
	uuid: string;
	user: ApiUserSimple;
	outdated: boolean;
	issue: boolean;
	resolved: boolean;
	thread_id: string | null;
	comment: string;
	mentions: ApiUserSimple[];
};

export type ApiChatMessage = {
	comment: string;
	mentions: ApiUserSimple[];
	emoji_reactions: ApiChatMessageReaction[];
	in_reply_to: ApiChatMessageInReplyTo | null;
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

export type ChatMessageReaction = {
	reaction: string;
	users: UserSimple[];
};

export function apiToChatMessageReaction(apiReaction: ApiChatMessageReaction): ChatMessageReaction {
	return {
		reaction: apiReaction.reaction,
		users: apiReaction.users.map(apiToUserSimple)
	};
}

export type ChatMessageInReplyTo = {
	uuid: string;
	user: UserSimple;
	outdated: boolean;
	issue: boolean;
	resolved: boolean;
	threadId: string | undefined;
	comment: string;
	mentions: UserSimple[];
};

export function apiToChatMessageInReplyTo(
	apiInReplyTo: ApiChatMessageInReplyTo
): ChatMessageInReplyTo {
	return {
		uuid: apiInReplyTo.uuid,
		user: apiToUserSimple(apiInReplyTo.user),
		outdated: apiInReplyTo.outdated,
		issue: apiInReplyTo.issue,
		resolved: apiInReplyTo.resolved,
		threadId: apiInReplyTo.thread_id ?? undefined,
		comment: apiInReplyTo.comment,
		mentions: apiInReplyTo.mentions.map(apiToUserSimple)
	};
}

export type ChatMessage = {
	comment: string;
	mentions: UserSimple[];
	inReplyTo: ChatMessageInReplyTo | undefined;
	emojiReactions: ChatMessageReaction[];
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
		inReplyTo: apiChatMessage.in_reply_to
			? apiToChatMessageInReplyTo(apiChatMessage.in_reply_to)
			: undefined,
		emojiReactions: apiChatMessage.emoji_reactions.map(apiToChatMessageReaction),
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
	/**
	 * UUID of chat message to reply to
	 */
	in_reply_to?: string;
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
	/**
	 * UUID of chat message to reply to
	 */
	inReplyTo?: string;
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
		range: params.range,
		in_reply_to: params.inReplyTo
	};
}

export type ApiPatchChatMessageParams = {
	chat_uuid: string;
	/**
	 * Chat message issue is resolved
	 */
	resolved?: boolean;
	/**
	 * Emoji reaction to add or, if present, remove
	 */
	reaction?: string;
};

export type PatchChatMessageParams = {
	projectId: string;
	changeId?: string;
	messageUuid: string;
	/**
	 * Chat message issue is resolved
	 */
	resolved?: boolean;
	/**
	 * Emoji reaction to add or, if present, remove
	 */
	reaction?: string;
};

export function toApiPatchChatMessageParams(
	params: PatchChatMessageParams
): ApiPatchChatMessageParams {
	return {
		chat_uuid: params.messageUuid,
		resolved: params.resolved,
		reaction: params.reaction
	};
}
