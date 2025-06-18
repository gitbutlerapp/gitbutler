import { BaseParamsSchema, DiffSpec } from './shared.js';
import { listStackBranches, listStacks } from './status.js';
import { executeGitButlerCommand, hasGitButlerExecutable } from '../../shared/command.js';
import { CreateCommitOutcomeSchema } from '../../shared/entities/stacks.js';
import { CallToolResult, GetPromptResult } from '@modelcontextprotocol/sdk/types.js';
import { z } from 'zod';
import { zodToJsonSchema } from 'zod-to-json-schema';

/**
 * Populate the diff spec for the commit command.
 */
function populateDiffSpec(
	params: {
		project_directory: string;
		message: string;
		all: boolean;
		filePaths: string[];
		branch: string;
	},
	args: string[]
) {
	if (params.all) {
		if (params.filePaths.length > 0) {
			throw new Error('Cannot use --all and file paths together');
		}

		return;
	}

	const diffSpec: DiffSpec[] = [];
	for (const filePath of params.filePaths) {
		diffSpec.push({
			pathBytes: filePath,
			hunkHeaders: []
		});
	}

	const diffSpecJson = JSON.stringify(diffSpec);
	args.push('--diff-spec', diffSpecJson);
}

/**
 * Generates a summary string describing the reasons for rejected changes and the associated file paths.
 *
 * Groups the rejected changes by their reason and lists the file paths for each reason.
 * The resulting string has the format: "reason1: file1, file2; reason2: file3".
 *
 * @param outcome - The outcome object containing paths to rejected changes, where each entry is a tuple of [reason, filePath].
 * @returns A summary string listing reasons and their corresponding file paths.
 */
function createRejectionSummary(outcome: CreateCommitOutcome): string {
	const groupedRejections = new Map<string, Set<string>>();
	for (const path of outcome.pathsToRejectedChanges) {
		const [reason, filePath] = path;
		const paths = groupedRejections.get(reason) || new Set<string>();
		paths.add(filePath);
		groupedRejections.set(reason, paths);
	}

	const rejectionMessages = Array.from(groupedRejections.entries())
		.map(([reason, paths]) => `${reason}: ${Array.from(paths).join(', ')}`)
		.join('; ');
	return rejectionMessages;
}

type CreateCommitOutcome = z.infer<typeof CreateCommitOutcomeSchema>;

function interpretOutcome(outcome: CreateCommitOutcome, action: 'created' | 'amend'): string {
	// Success
	if (outcome.newCommit !== null && outcome.pathsToRejectedChanges.length === 0) {
		return `Commit successfully ${action} with ID ${outcome.newCommit}`;
	}

	// Created commit but some changes were rejected
	if (outcome.newCommit !== null && outcome.pathsToRejectedChanges.length > 0) {
		const rejectionMessages = createRejectionSummary(outcome);

		return `Commit successfully ${action} with ID ${outcome.newCommit}, but some changes were rejected: ${rejectionMessages}`;
	}

	// No commit created
	let message = `No commit could be ${action}`;
	if (outcome.pathsToRejectedChanges.length > 0) {
		const rejectionMessages = createRejectionSummary(outcome);
		message += `, and the following changes were rejected: ${rejectionMessages}`;
	}

	return message;
}

const CommitParamsSchema = BaseParamsSchema.extend({
	message: z.string({ description: 'The commit message' }),
	all: z.boolean().optional().default(false),
	filePaths: z
		.array(z.string(), {
			description: 'The paths of files to commit. These have to be relative paths.'
		})
		.optional()
		.default([]),
	branch: z.string({ description: 'The branch to commit to' })
});

type CommitParams = z.infer<typeof CommitParamsSchema>;

type AmendCommitParams = {
	/**
	 * The commit ID to amend.
	 */
	commitId: string;
	/**
	 * The commit message to use.
	 */
	message?: string;
};

/**
 * Commit changes.
 */
function commit(params: CommitParams, amendParams?: AmendCommitParams) {
	const args = ['commit'];

	if (amendParams !== undefined) {
		args.push('--amend', '--parent', amendParams.commitId);
		if (amendParams.message) {
			args.push('--message', amendParams.message);
		}
		return executeGitButlerCommand(params.project_directory, args, CreateCommitOutcomeSchema);
	}

	args.push('--message', params.message);

	populateDiffSpec(params, args);

	const stacks = listStacks({ project_directory: params.project_directory });

	if (stacks.length === 0) {
		// TODO: Would be nice to create a new stack if there are no stacks
		throw new Error('No stacks found');
	}

	for (const stack of stacks) {
		const heads = stack.heads.map((h) => h.name);
		if (heads.includes(params.branch)) {
			if (heads.length === 1) {
				// If this stack has only one branch, we can commit directly to it
				args.push('-s', params.branch);
				return executeGitButlerCommand(params.project_directory, args, CreateCommitOutcomeSchema);
			}

			const stackBranches = listStackBranches({
				project_directory: params.project_directory,
				stack_id: stack.id
			});
			if (stackBranches.length === 0) {
				throw new Error(`No branches found in stack ${stack.id}`);
			}

			const branch = stackBranches.find((b) => b.name === params.branch);
			if (!branch) {
				throw new Error(`Branch ${params.branch} not found in stack ${stack.id}`);
			}

			if (branch.archived) {
				throw new Error(`Branch ${params.branch} is archived`);
			}

			args.push('-s', params.branch);
			return executeGitButlerCommand(params.project_directory, args, CreateCommitOutcomeSchema);
		}
	}

	throw new Error(`Branch ${params.branch} not found in any stack`);
}

const DraftCommitsParamsSchema = BaseParamsSchema.extend({
	branch: z.string({ description: 'The branch to commit to' }),
	commitMessages: z.array(z.string({ description: 'The commit description' }), {
		description: 'The list of commits in the order they will be committed'
	})
});

type DraftCommitsParams = z.infer<typeof DraftCommitsParamsSchema>;

/**
 * Create a set of empty commits in the specified branch.
 */
function draftCommits(params: DraftCommitsParams) {
	for (const commitMessage of params.commitMessages) {
		const message = commitMessage;
		commit({
			project_directory: params.project_directory,
			branch: params.branch,
			message,
			all: false,
			filePaths: []
		});
	}
}

const AmmendCommitParamsSchema = BaseParamsSchema.extend({
	commitId: z.string({ description: 'The commit ID to amend' }),
	all: z.boolean().optional().default(false),
	filePaths: z
		.array(z.string(), {
			description: 'The paths of files to add to the commit. These have to be relative paths.'
		})
		.optional()
		.default([]),
	branch: z.string({ description: 'The branch to amend commits in' }),
	message: z
		.string({ description: 'Optional message update. If undefined, the same message will be kept' })
		.optional()
});

type AmmendCommitParams = z.infer<typeof AmmendCommitParamsSchema>;

/**
 * Ammend a commit.
 */
function amendCommit(params: AmmendCommitParams) {
	return commit(
		{
			project_directory: params.project_directory,
			branch: params.branch,
			message: '',
			all: params.all,
			filePaths: params.filePaths
		},
		{
			commitId: params.commitId,
			message: params.message
		}
	);
}

const TOOL_LISTINGS = [
	{
		name: 'commit',
		description: 'Commit a set of changes to a specific branch in the GitButler project.',
		inputSchema: zodToJsonSchema(CommitParamsSchema)
	},
	{
		name: 'create-draft-commits',
		description:
			'Create a set of empty commits in the specified branch as drafts or placeholders for upcoming changes.',
		inputSchema: zodToJsonSchema(DraftCommitsParamsSchema)
	},
	{
		name: 'amend-commit',
		description: 'Add files to an existing commit in a specified branch, amending it.',
		inputSchema: zodToJsonSchema(AmmendCommitParamsSchema)
	}
] as const;

type ToolName = (typeof TOOL_LISTINGS)[number]['name'];

function isToolName(name: string): name is ToolName {
	return TOOL_LISTINGS.some((tool) => tool.name === name);
}

export function getCommitToolListing() {
	if (!hasGitButlerExecutable()) {
		return [];
	}

	return TOOL_LISTINGS;
}

export async function getCommitToolRequestHandler(
	toolName: string,
	params: Record<string, unknown>
): Promise<CallToolResult | null> {
	if (!isToolName(toolName) || !hasGitButlerExecutable()) {
		return null;
	}

	try {
		switch (toolName) {
			case 'commit': {
				const parsedParams = CommitParamsSchema.parse(params);
				const outcome = commit(parsedParams);
				const interpretation = interpretOutcome(outcome, 'created');
				return { content: [{ type: 'text', text: interpretation }] };
			}
			case 'create-draft-commits': {
				const parsedParams = DraftCommitsParamsSchema.parse(params);
				draftCommits(parsedParams);
				return { content: [{ type: 'text', text: 'Draft commits created successfully' }] };
			}
			case 'amend-commit': {
				const parsedParams = AmmendCommitParamsSchema.parse(params);
				const outcome = amendCommit(parsedParams);
				const interpretation = interpretOutcome(outcome, 'amend');
				return { content: [{ type: 'text', text: interpretation }] };
			}
		}
	} catch (error: unknown) {
		if (error instanceof Error) {
			return { content: [{ type: 'text', text: `Error: ${error.message}` }], isError: true };
		}

		return { content: [{ type: 'text', text: `Error: ${String(error)}` }], isError: true };
	}
}

const PROMPTS = [
	{
		name: 'commit',
		description: `Commit the file changes into the right stack and branch.
This will create and propose a commit plan for the changes in the project.
If there's any ambiguity, will ask the user for clarification.`,
		arguments: [
			{
				name: 'disambiguation',
				description:
					'Any kind of additional information that can help to disambiguate what and how the commits should be done.',
				required: false
			}
		]
	},
	{
		name: 'absorb',
		description: `Absorb the file changes into the right stack and branch.
This will attempt to amend the uncommitted changes in the project to the right branch and commit.
If there's any ambiguity, will ask the user for clarification.`,
		arguments: [
			{
				name: 'disambiguation',
				description:
					'Any kind of additional information that can help to disambiguate what and how the commits should be done.',
				required: false
			}
		]
	}
] as const;

function isCommitPromptParams(
	params: Record<string, unknown>
): params is { disambiguation?: string } {
	return typeof params.disambiguation === 'string' || typeof params.disambiguation === 'undefined';
}

function buildCommitPrompt(params: Record<string, unknown>): GetPromptResult {
	const disambiguation = isCommitPromptParams(params) ? params.disambiguation : undefined;
	const suffix = disambiguation ? '\nImportantly: ' + disambiguation : '';

	return {
		messages: [
			{
				role: 'user',
				content: {
					type: 'text',
					text: `I want to commit the changes in my project using GitButler.
Follow these instructions to do so:
1. List and take a look at the file changes in the project.
2. Determine to which branch (or branches) to commit what. For that, you can list the stacks and branches in the project and take a look at their names.
3. Create a commit plan for the changes. By commit plan, I mean a list of commits that will be created based off the changes listed above.
4. Propose the commit plan to me, including the target branch (or branches), commit messages and the files that will be included in each commit.
5. If there's any ambiguity, ask me for clarification.
6. If I accept, commit as planned.${suffix}`
				}
			}
		]
	};
}

function buildAbsorbPrompt(params: Record<string, unknown>): GetPromptResult {
	const disambiguation = isCommitPromptParams(params) ? params.disambiguation : undefined;
	const suffix = disambiguation ? '\nImportantly: ' + disambiguation : '';

	return {
		messages: [
			{
				role: 'user',
				content: {
					type: 'text',
					text: `I want to absorb the changes in my project using GitButler.
Follow these instructions to do so:
1. List and take a look at the file changes in the project.
2. Determine to which branch (or branches) to absorb the changes. For that, you can list the stacks and branches in the project and take a look at their names.
3. Determine the commit that will be amended with the changes. For that, you can list the commits in the branch and take a look at their messages.
4. Propose the commit plan to me, including the target branch (or branches), commit messages to be updated and the files that will be included in each commit.
5. If there's any ambiguity, ask me for clarification.
6. If I accept, amend the commit as planned.${suffix}`
				}
			}
		]
	};
}

type PromptName = (typeof PROMPTS)[number]['name'];

function isPromptName(name: string): name is PromptName {
	return PROMPTS.some((prompt) => prompt.name === name);
}

export function getCommitToolPrompts() {
	if (!hasGitButlerExecutable()) {
		return [];
	}

	return PROMPTS;
}

export async function getCommitToolPromptRequestHandler(
	promptName: string,
	params: Record<string, unknown>
): Promise<GetPromptResult | null> {
	if (!isPromptName(promptName) || !hasGitButlerExecutable()) {
		return null;
	}

	switch (promptName) {
		case 'commit': {
			return buildCommitPrompt(params);
		}
		case 'absorb': {
			return buildAbsorbPrompt(params);
		}
	}
}
