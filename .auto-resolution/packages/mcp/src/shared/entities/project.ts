import { z } from 'zod';

export const ProjectSimpleSchema = z.object({
	name: z.string({ description: 'The name of the project' }),
	description: z.string({ description: 'The description of the project' }).nullable(),
	slug: z.string({ description: 'The slug of the project' }),
	owner: z.string({ description: 'The owner of the project' }),
	full_slug: z.string({ description: 'The full slug of the project' }),
	last_push_at: z.string({ description: 'The last time the project was pushed to' }).optional(),
	creted_at: z.string({ description: 'The time the project was created' }).optional(),
	updated_at: z.string({ description: 'The time the project was updated' }).optional()
});

export type ProjectSimple = z.infer<typeof ProjectSimpleSchema>;

export const ProjectSchema = ProjectSimpleSchema.extend({
	readme: z.string({ description: 'The README of the project' }).nullable(),
	repository_id: z.string({ description: 'The ID of the repository' }),
	code_repository_id: z.string({ description: 'The ID of the code repository' }),
	git_url: z.string({ description: 'The git URL of the project' }).optional(),
	code_git_url: z.string({ description: 'The code git URL of the project' }).optional(),
	git_ssh_url: z.string({ description: 'The git SSH URL of the project' }).optional(),
	code_git_ssh_url: z.string({ description: 'The code git SSH URL of the project' }).optional(),
	active_reviews_count: z.number({ description: 'The number of active reviews' }).optional(),
	ai_code_review_enabled: z.boolean({ description: 'Whether AI code review is enabled' }).optional()
});

export type Project = z.infer<typeof ProjectSchema>;

export const LookupProjectResponseSchema = z.object({
	repository_id: z.string({ description: 'The ID of the repository' })
});

export type LookupProjectResponse = z.infer<typeof LookupProjectResponseSchema>;
