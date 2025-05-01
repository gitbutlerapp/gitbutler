import { UserMaybeSchema, UserSimpleSchema } from './user.js';
import { z } from 'zod';

export const PatchReviewSchema = z.object({
	viewed: UserSimpleSchema.array(),
	signed_off: UserSimpleSchema.array(),
	rejected: UserSimpleSchema.array()
});

export type PatchReview = z.infer<typeof PatchReviewSchema>;

export const PatchStatisticsSchema = z.object({
	file_count: z.number({ description: 'The number of files modified in the patch' }),
	section_count: z.number({ description: 'The number of sections in the patch' }),
	lines: z.number({ description: 'The number of lines affected in the patch' }),
	additions: z.number({ description: 'The number of lines added in the patch' }),
	deletions: z.number({ description: 'The number of lines deleted in the patch' }),
	files: z.array(z.string({ description: 'The file name' }), {
		description: 'The files modified in the patch'
	})
});

export type PatchStatistics = z.infer<typeof PatchStatisticsSchema>;

export const PatchSchema = z.object({
	change_id: z.string({ description: 'The ID of the change' }),
	commit_sha: z.string({ description: 'The SHA of the commit' }),
	patch_sha: z.string({ description: 'The SHA of the patch' }),
	title: z.string({ description: 'The title of the patch' }),
	description: z.string({ description: 'The description of the patch' }).nullable(),
	position: z.number({ description: 'The position of the patch' }),
	version: z.number({ description: 'The version of the patch' }),
	created_at: z.string({ description: 'The time the patch was created' }).optional(),
	updated_at: z.string({ description: 'The time the patch was updated' }).optional(),
	review_status: z
		.enum(['unreviewed', 'approved', 'changes-requested', 'in-discussion'], {
			description: 'The review status of the patch'
		})
		.optional(),
	contributors_array: UserMaybeSchema.array().nullable().optional(),
	statistics: PatchStatisticsSchema,
	comment_count: z.number({ description: 'The number of comments on the patch' }).nullable(),
	review: PatchReviewSchema.nullable(),
	review_all: PatchReviewSchema.nullable(),
	branch_uuid: z.string({ description: 'The UUID of the branch' }).nullable(),
	type: z.string({ description: 'The type of the patch' }),
	patch_id: z.string({ description: 'The ID of the patch' }).nullable()
});

export type Patch = z.infer<typeof PatchSchema>;

export const PatchTextSectionSchema = z.object({
	id: z.number({ description: 'The ID of the section' }),
	sectionType: z.literal('text', {
		description: 'The type of the section'
	}),
	identifier: z.string({ description: 'The identifier of the section' }),
	title: z.string({ description: 'The title of the section' }).optional(),
	position: z.number({ description: 'The position of the section' }).optional()
});

export type PatchTextSection = z.infer<typeof PatchTextSectionSchema>;

export const PatchDiffSectionSchema = z.object({
	id: z.number({ description: 'The ID of the section' }),
	section_type: z.literal('diff', {
		description: 'The type of the section'
	}),
	identifier: z.string({ description: 'The identifier of the section' }),
	title: z.string({ description: 'The title of the section' }).optional(),
	position: z.number({ description: 'The position of the section' }).optional(),
	diff_sha: z.string({ description: 'The SHA of the diff' }),
	base_file_sha: z.string({ description: 'The SHA of the base file' }),
	new_file_sha: z.string({ description: 'The SHA of the new file' }),
	old_path: z.string({ description: 'The path of the old file' }).optional(),
	old_size: z.number({ description: 'The size of the old file' }).optional(),
	new_path: z.string({ description: 'The path of the new file' }).optional(),
	new_size: z.number({ description: 'The size of the new file' }).optional(),
	hunks: z.number({ description: 'The number of hunks in the diff' }).optional(),
	lines: z.number({ description: 'The number of lines in the diff' }).optional(),
	deletions: z.number({ description: 'The number of lines deleted in the diff' }).optional(),
	diff_patch: z.string({ description: 'The diff patch' }).optional()
});

export const PatchWithFilesSchema = PatchSchema.extend({
	sections: z.union([PatchTextSectionSchema, PatchDiffSectionSchema]).array()
});

export type PatchWithFiles = z.infer<typeof PatchWithFilesSchema>;
