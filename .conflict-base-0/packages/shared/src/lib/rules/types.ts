import type { LoadableData } from '$lib/network/types';

export type ApiRule = {
	uuid: string;
	title: string;
	description: string;
	project_slug: string;
	negative_example: string;
	positive_example: string;
	created_at: string;
	updated_at: string;
};

export type Rule = {
	uuid: string;
	title: string;
	description: string;
	projectSlug: string;
	negativeExample: string;
	positiveExample: string;
	createdAt: string;
	updatedAt: string;
};

export function apiToRule(apiRule: ApiRule): Rule {
	return {
		uuid: apiRule.uuid,
		title: apiRule.title,
		description: apiRule.description,
		projectSlug: apiRule.project_slug,
		negativeExample: apiRule.negative_example,
		positiveExample: apiRule.positive_example,
		createdAt: apiRule.created_at,
		updatedAt: apiRule.updated_at
	};
}

export type LoadableRule = LoadableData<Rule, Rule['uuid']>;

/**
 * Parameters for creating a new rule.
 */
export type CreateRuleParams = {
	projectSlug: string;
	title: string;
	description: string;
	negativeExample?: string;
	positiveExample?: string;
};

export type ApiCreateRuleParams = {
	project_slug: string;
	title: string;
	description: string;
	negative_example?: string;
	positive_example?: string;
};

export function toApiCreateRuleParams(params: CreateRuleParams): ApiCreateRuleParams {
	return {
		project_slug: params.projectSlug,
		title: params.title,
		description: params.description,
		negative_example: params.negativeExample,
		positive_example: params.positiveExample
	};
}
