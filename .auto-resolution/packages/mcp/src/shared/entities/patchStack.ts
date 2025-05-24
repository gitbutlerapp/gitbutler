import { PatchSchema } from './patch.js';
import { UserMaybeSchema } from './user.js';
import { z } from 'zod';

export const PatchStackSimpleSchema = z.object({
	uuid: z.string({ description: 'The UUID of the patch stack' }),
	branch_id: z.string({ description: 'The ID of the branch' }),
	project_full_slug: z.string({
		description: 'The full slug of the project the patch stack belonds to'
	}),
	stack_size: z.number({ description: 'The size of the patch stack' }),
	updated_at: z.string({ description: 'The time the patch stack was updated' }).optional(),
	contributors: UserMaybeSchema.array(),
	review_status: z.enum(['unreviewed', 'approved', 'changes-requested', 'in-discussion'], {
		description: 'The review status of the patch stack'
	}),
	version: z.number({ description: 'The version of the patch stack' }),
	title: z.string({ description: 'The title of the patch stack' }),
	status: z.enum(['active', 'inactive', 'closed', 'previous', 'loading'], {
		description: 'The status of the patch stack'
	}),
	review_url: z.string({ description: 'The URL of the review' }).nullable()
	// permissions
});

export type PatchStackSimple = z.infer<typeof PatchStackSimpleSchema>;

export const PatchStackSchema = PatchStackSimpleSchema.extend({
	owner_login: z.string({ description: 'The username of the owner of the patch stack' }).optional(),
	oplog_sha: z.string({ description: 'The SHA of the oplog' }).nullable().optional(),
	description: z.string({ description: 'The description of the patch stack' }).optional(),
	reviewers: UserMaybeSchema.array().optional(),
	repository_id: z.string({ description: 'The ID of the repository' }).optional(),
	branch_stack_id: z.string({ description: 'The ID of the branch stack' }).optional(),
	branch_stack_order: z.number({ description: 'The order of the branch stack' }).optional(),
	patches: PatchSchema.array().optional(),
	forge_description: z.string({ description: 'The description of the patch stack' }).optional(),
	forge_url: z.string({ description: 'The URL of the patch stack' }).optional(),
	created_at: z.string({ description: 'The time the patch stack was created' }).optional()
});

export type PatchStack = z.infer<typeof PatchStackSchema>;
