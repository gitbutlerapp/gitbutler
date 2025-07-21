import {
	createMutationEndpoint,
	createQueryEndpointWithTransform,
	type CustomBuilder,
	type EndpointMap
} from '$lib/state/butlerModule';
import { invalidatesItem, invalidatesList, providesItems, ReduxTag } from '$lib/state/tags';
import { createEntityAdapter, type EntityState } from '@reduxjs/toolkit';
import type {
	CreateRuleRequest,
	UpdateRuleRequest,
	WorkspaceRule,
	WorkspaceRuleId
} from '$lib/rules/rule';
import type { BackendApi } from '$lib/state/clientState.svelte';

function typedEntries<T extends Record<string, unknown>>(obj: T): [keyof T, T[keyof T]][] {
	return Object.entries(obj) as [keyof T, T[keyof T]][];
}

function typedFromEntries<T extends Record<string, unknown>>(entries: [keyof T, T[keyof T]][]): T {
	return Object.fromEntries(entries) as T;
}

export default class BackendService {
	private static instance: BackendService;
	private mutationApi: ReturnType<typeof injectMutationEndpoints>;
	private queryApi: ReturnType<typeof injectQueryEndpoints>;

	private constructor(backendApi: BackendApi) {
		this.mutationApi = injectMutationEndpoints(backendApi);
		this.queryApi = injectQueryEndpoints(backendApi);
	}

	static getInstance(backendApi: BackendApi): BackendService {
		if (!BackendService.instance) {
			BackendService.instance = new BackendService(backendApi);
		}
		return BackendService.instance;
	}

	get() {
		// Mutations
		type MutationEndpoints = typeof this.mutationApi.endpoints;

		type MutateMap = {
			[K in keyof MutationEndpoints as `${K}Mutate`]: MutationEndpoints[K]['mutate'];
		};

		type UseMutationMap = {
			[K in keyof MutationEndpoints as `${K}UseMutation`]: MutationEndpoints[K]['useMutation'];
		};

		const mutate = typedFromEntries(
			typedEntries(this.mutationApi.endpoints).map(
				([key, value]) => [`${key}Mutate`, value.mutate] as const
			)
		) as MutateMap;

		const useMutation = typedFromEntries(
			typedEntries(this.mutationApi.endpoints).map(
				([key, value]) => [`${key}UseMutation`, value.useMutation] as const
			)
		) as UseMutationMap;

		// Queries
		type QueryEndpoints = typeof this.queryApi.endpoints;

		type UseQueryMap = {
			[K in keyof QueryEndpoints as `${K}UseQuery`]: (typeof this.queryApi.endpoints)[K]['useQuery'];
		};

		type FetchMap = {
			[K in keyof QueryEndpoints as `${K}Fetch`]: (typeof this.queryApi.endpoints)[K]['fetch'];
		};

		const useQuery = typedFromEntries(
			typedEntries(this.queryApi.endpoints).map(
				([key, value]) => [`${key}UseQuery`, value.useQuery] as const
			)
		) as UseQueryMap;

		const fetchMap = typedFromEntries(
			typedEntries(this.queryApi.endpoints).map(
				([key, value]) => [`${key}Fetch`, value.fetch] as const
			)
		) as FetchMap;

		return {
			...mutate,
			...useMutation,
			...useQuery,
			...fetchMap
		};
	}
}

function injectQueryEndpoints(api: BackendApi) {
	return api.injectEndpoints({
		endpoints: (build) => getQueryEndpointMap(build)
	});
}

function injectMutationEndpoints(api: BackendApi) {
	return api.injectEndpoints({
		endpoints: (build) => getMutationEndpointMap(build)
	});
}

function getMutationEndpointMap(builder: CustomBuilder) {
	return {
		createWorkspaceRule: createMutationEndpoint<
			WorkspaceRule,
			{ projectId: string; request: CreateRuleRequest }
		>(builder, 'create_workspace_rule', () => [invalidatesList(ReduxTag.WorkspaceRules)]),
		deleteWorkspaceRule: createMutationEndpoint<
			void,
			{ projectId: string; ruleId: WorkspaceRuleId }
		>(builder, 'delete_workspace_rule', () => [invalidatesList(ReduxTag.WorkspaceRules)]),
		updateWorkspaceRule: createMutationEndpoint<
			WorkspaceRule,
			{ projectId: string; request: UpdateRuleRequest }
		>(builder, 'update_workspace_rule', (result) =>
			result
				? [
						invalidatesItem(ReduxTag.WorkspaceRules, result.id),
						invalidatesList(ReduxTag.WorkspaceRules)
					]
				: []
		)
	} satisfies EndpointMap;
}

function getQueryEndpointMap(builder: CustomBuilder) {
	return {
		listWorkspaceRules: createQueryEndpointWithTransform<
			WorkspaceRule[],
			{ projectId: string },
			EntityState<WorkspaceRule, WorkspaceRuleId>
		>(
			builder,
			'list_workspace_rules',
			(response: WorkspaceRule[]) => {
				return workspaceRulesAdapter.addMany(workspaceRulesAdapter.getInitialState(), response);
			},
			(result) => providesItems(ReduxTag.WorkspaceRules, result?.ids ?? [])
		)
	} satisfies EndpointMap;
}

const workspaceRulesAdapter = createEntityAdapter<WorkspaceRule, WorkspaceRuleId>({
	selectId: (rule) => rule.id
});

export const workspaceRulesSelectors = workspaceRulesAdapter.getSelectors();
