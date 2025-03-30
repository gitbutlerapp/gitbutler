import { ChatMessageSchema } from '../shared/entities/chatMessage.js';
import { getGitbutlerAPIUrl, gitbutlerAPIRequest, interpolatePath } from '../shared/request.js';
import { z } from 'zod';

enum ChatMessageAPIEndpoint {
	ChatMessages = '/chat_messages/{projectId}/chats/{changeId}'
}

export const GetChatMessagesForPatchParamsSchema = z.object({
	projectId: z.string({ description: 'The ID of the project' }),
	changeId: z.string({ description: 'The ID of the change' }),
	limit: z.number({ description: 'Limit the number of results listed' }).optional(),
	before: z.string({ description: 'Get messages before this date' }).optional(),
	since: z.string({ description: 'Get messages after this date' }).optional()
});

export type GetChatMessagesForPatchParams = z.infer<typeof GetChatMessagesForPatchParamsSchema>;

/**
 * Return all chat messages for a patch
 */
export async function getChatMessagesForPatch(params: GetChatMessagesForPatchParams) {
	const interpolationParams = {
		projectId: params.projectId,
		changeId: params.changeId
	};

	const queryParams = {
		limit: params.limit,
		before: params.before,
		since: params.since
	};

	const apiPath = interpolatePath(ChatMessageAPIEndpoint.ChatMessages, interpolationParams);
	const url = getGitbutlerAPIUrl(apiPath, queryParams);
	const response = await gitbutlerAPIRequest(url);
	const parsed = ChatMessageSchema.array().parse(response);
	return parsed;
}
