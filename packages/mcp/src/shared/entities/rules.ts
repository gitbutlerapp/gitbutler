import { z } from 'zod';

export const RuleSchema = z.object({
	uuid: z.string().uuid(),
	title: z.string().min(1, { message: 'Title is required' }),
	description: z.string().min(1, { message: 'Description is required' }),
	project_slug: z.string().min(1, { message: 'Project slug is required' }),
	negative_example: z.string().optional(),
	positive_example: z.string().optional(),
	createdAt: z.string().datetime({ message: 'Created at must be a valid date' }),
	updatedAt: z.string().datetime({ message: 'Updated at must be a valid date' })
});

export type Rule = z.infer<typeof RuleSchema>;
