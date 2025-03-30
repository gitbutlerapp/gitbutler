import { UserSimpleSchema } from './user.js';
import { z } from 'zod';

export const DiffPatchSchema = z.object({
	type: z.enum(['context', 'added', 'removed'], {
		description: 'The type of the diff patch'
	}),
	left: z.number({ description: 'The left line number' }).optional(),
	right: z.number({ description: 'The right line number' }).optional(),
	line: z.string({ description: 'The line of the diff patch' })
});

export type DiffPatch = z.infer<typeof DiffPatchSchema>;

export const ChatMessageReactionSchema = z.object({
	reaction: z.string({ description: 'The emoji reaction' }),
	users: UserSimpleSchema.array()
});

export type ChatMessageReaction = z.infer<typeof ChatMessageReactionSchema>;

export const ChatMessageInReplyToSchema = z.object({
	uuid: z.string({ description: 'The UUID of the message' }),
	user: UserSimpleSchema,
	outdated: z.boolean({ description: 'Whether the message is outdated' }),
	issue: z.boolean({ description: 'Whether the message is an issue' }),
	resolved: z.boolean({ description: 'Whether the message is resolved' }),
	thread_id: z.string({ description: 'The ID of the thread' }).nullable(),
	comment: z.string({ description: 'The text of the message' }),
	mentions: UserSimpleSchema.array()
});

export const ChatMessageSchema = z.object({
	uuid: z.string({ description: 'The UUID of the message' }),
	user: UserSimpleSchema,
	outdated: z.boolean({ description: 'Whether the message is outdated' }),
	issue: z.boolean({ description: 'Whether the message is an issue' }),
	resolved: z.boolean({ description: 'Whether the message is resolved' }),
	thread_id: z.string({ description: 'The ID of the thread' }).nullable(),
	comment: z.string({ description: 'The text of the message' }),
	emoji_reactions: ChatMessageReactionSchema.array(),
	mentions: UserSimpleSchema.array(),
	in_reply_to: ChatMessageInReplyToSchema.nullable(),
	diff_sha: z
		.string({ description: 'The SHA of the diff this message is quoting, if any' })
		.nullable(),
	range: z
		.string({ description: 'The range of lines in the diff this message is quoting, if any' })
		.nullable(),
	diff_path: z
		.string({ description: 'The file path of the diff this message is quoting, if any' })
		.nullable(),
	diff_patch_array: DiffPatchSchema.array().nullable(),
	created_at: z.string({ description: 'The time the message was created' }).optional(),
	updated_at: z.string({ description: 'The time the message was updated' }).optional()
});

export type ChatMessage = z.infer<typeof ChatMessageSchema>;
