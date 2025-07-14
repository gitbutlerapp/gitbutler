import { RuleSchema } from '../shared/entities/index.js';
import {
	getGitbutlerAPIUrl,
	gitbutlerAPIRequest,
	hasGitButlerAPIKey,
	interpolatePath
} from '../shared/request.js';
import { CallToolResult } from '@modelcontextprotocol/sdk/types.js';
import { z } from 'zod';
import { zodToJsonSchema } from 'zod-to-json-schema';

enum RuleAPIEndpoint {
	Rules = '/rules',
	Rule = '/rules/{uuid}',
	ProjectRules = '/rules/project/{project_slug}'
}

const ListRulesParamsSchema = z.object({
	context: z.string({
		description:
			'Information about the context in which the rules are being listed, e.g. code intended to be generated, relevant code snippets. Shuold be a detailed as possible.'
	}),
	project_slug: z.string({ description: 'The slug of the project to list rules for' })
});

type ListRulesParams = z.infer<typeof ListRulesParamsSchema>;

async function listRules(params: ListRulesParams) {
	const apiPath = interpolatePath(RuleAPIEndpoint.ProjectRules, {
		project_slug: params.project_slug
	});
	const url = getGitbutlerAPIUrl(apiPath, { context: params.context });
	const response = await gitbutlerAPIRequest(url);
	const parsed = RuleSchema.array().parse(response);
	return parsed;
}

const TOOL_LISTINGS = [
	{
		name: 'get_relevant_rules',
		description: `
        <description>
            Get instructions for how to generate code based on the rules defined in the project.
            This tool will return a list of rules that are relevant to the context provided.
        </description>

        <important_note>
            It's important to pass detaild information about the context in which the rules are being requested.
            The more detailed the context, the more accurate the rules will be.
            If used for reviewing code, provide the code snippets that are relevant to the rules.
        </important_note>
        `,
		inputSchema: zodToJsonSchema(ListRulesParamsSchema)
	}
] as const;

type ToolName = (typeof TOOL_LISTINGS)[number]['name'];

function isToolName(name: string): name is ToolName {
	return TOOL_LISTINGS.some((tool) => tool.name === name);
}

export function getRulesToolListings() {
	if (!hasGitButlerAPIKey()) {
		return [];
	}
	return TOOL_LISTINGS;
}

export async function getRulesToolRequestHandler(
	toolName: string,
	args: Record<string, unknown>
): Promise<CallToolResult | null> {
	if (!isToolName(toolName) || !hasGitButlerAPIKey()) {
		return null;
	}
	switch (toolName) {
		case 'get_relevant_rules': {
			const params = ListRulesParamsSchema.parse(args);
			const rules = await listRules(params);
			return {
				content: [{ type: 'text', text: JSON.stringify(rules, null, 2) }]
			};
		}
	}
}
