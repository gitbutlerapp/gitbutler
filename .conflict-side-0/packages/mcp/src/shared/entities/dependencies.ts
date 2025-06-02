import { DiffHunkSchema } from './changes.js';
import { z } from 'zod';

const HunkLockSchema = z.object({
	stackId: z.string({ description: 'The stack ID that this hunk is dependent on.' }),
	commitId: z.string({ description: 'The commit ID that this hunk is dependent on.' })
});

export type HunkLock = z.infer<typeof HunkLockSchema>;

const DiffDependency = z.tuple([
	z.string({ description: 'The path of the file.' }),
	DiffHunkSchema.describe('The diff hunk.'),
	z.array(HunkLockSchema).describe('The locks of the diff hunk.')
]);

export const HunkDependenciesSchema = z.object({
	diffs: z.array(DiffDependency).describe('The dependencies of the hunks in the diff.')
});
