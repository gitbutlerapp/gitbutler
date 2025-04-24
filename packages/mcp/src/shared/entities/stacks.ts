import { z } from 'zod';

export const StackSchema = z.object({
	id: z.string({ description: 'The unique identifier for the stack. This is a UUID.' }),
	branchNames: z.array(z.string()),
	tip: z.string({ description: 'The commit ID of the tip of the stack.' })
});

export const StackListSchema = z.array(StackSchema);

export const BranchSchema = z.object({
	name: z.string({ description: 'The name of the branch.' }),
	remoteTrackingBranch: z
		.string({ description: 'The name of the remote this branches will be pushed to.' })
		.nullable(),
	description: z.string({ description: 'Some information about the branch, if any.' }).nullable(),
	prNumber: z.number({ description: 'The associated Pull or Merge Request number.' }).nullable(),
	reviewId: z
		.string({
			description: 'The unique identifier for the GitButler recview associated with the branch.'
		})
		.nullable(),
	archived: z.boolean({
		description:
			'Indicates whether the branch is part of this stack, but has already been integrated. In other words, the merge base of the stack is above this branch.'
	}),
	tip: z.string({
		description:
			'The commit ID of the tip of the branch. If this is the only branch in the stack or the top-most branch, this is the tip of the stack.'
	}),
	baseCommit: z.string({
		description:
			'The commit the the branch is based on. If this branch is stacked on top of another, this is the head of the preceding branch. If this branch is the bottom-most branch of the stack, this is the merge base of the stack.'
	})
});

export const BranchListSchema = z.array(BranchSchema);

export const AuthorSchema = z.object({
	name: z.string({ description: 'The name of the author of the commit' }),
	email: z.string({ description: 'The email of the author of the commit' }),
	gravatarUrl: z.string({ description: 'The Gravatar URL of the author of the commit' })
});

export const CommitStateSchema = z.discriminatedUnion('type', [
	z.object({
		type: z.literal('LocalOnly')
	}),
	z.object({
		type: z.literal('LocalAndRemote'),
		subject: z.string({
			description:
				'The remote commit ID, which may differ if the local commit has been rebased or updated.'
		})
	}),
	z.object({
		type: z.literal('Integrated')
	})
]);

export const CommitSchema = z.object({
	id: z.string({ description: 'The commit SHA' }),
	parentIds: z.array(z.string({ description: 'The parent commit SHAs' })),
	message: z.string({ description: 'The commit message' }),
	hasConflicts: z.boolean({ description: 'Whether the commit has conflicts' }),
	createdAt: z.number({ description: 'The commit creation time in Epoch milliseconds' }),
	author: AuthorSchema,
	state: CommitStateSchema
});

export const UpstreamCommitSchema = z.object({
	id: z.string({ description: 'The commit SHA' }),
	message: z.string({ description: 'The commit message' }),
	createdAt: z.number({ description: 'The commit creation time in Epoch milliseconds' }),
	author: AuthorSchema
});

export const BranchCommitsSchema = z.object({
	localAndRemote: z.array(CommitSchema),
	upstreamCommits: z.array(UpstreamCommitSchema)
});
