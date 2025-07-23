import { InterestStore } from '$lib/interest/interestStore';
import { errorToLoadable } from '$lib/network/loadable';
import { POLLING_GLACIALLY } from '$lib/polling';
import { rulesTable } from '$lib/rules/rulesSlice';
import {
	apiToRule,
	toApiCreateRuleParams,
	type ApiRule,
	type CreateRuleParams,
	type LoadableRule
} from '$lib/rules/types';
import { InjectionToken } from '../context';
import type { HttpClient } from '$lib/network/httpClient';
import type { AppDispatch } from '$lib/redux/store.svelte';

const USER_RULE_LIST_KEY = 'me';

export const RULES_SERVICE_TOKEN = new InjectionToken<RulesService>('RulesService');

export class RulesService {
	private readonly rulesListInterest = new InterestStore<{ owner: string }>(POLLING_GLACIALLY);
	constructor(
		private readonly httpClient: HttpClient,
		private readonly appDispatch: AppDispatch
	) {}

	getRuleListInterest() {
		return this.rulesListInterest
			.findOrCreateSubscribable({ owner: USER_RULE_LIST_KEY }, async () => {
				const fakeKey = 'fake';
				this.appDispatch.dispatch(rulesTable.addOne({ status: 'loading', id: fakeKey }));
				try {
					const apiRules = await this.httpClient.get<ApiRule[]>('rules');
					const loadableRules: LoadableRule[] = apiRules.map((apiRule) => ({
						status: 'found',
						id: apiRule.uuid,
						value: apiToRule(apiRule)
					}));
					this.appDispatch.dispatch(rulesTable.upsertMany(loadableRules));
					this.appDispatch.dispatch(rulesTable.removeOne(fakeKey)); // Remove the fake loading entry
				} catch (error: unknown) {
					this.appDispatch.dispatch(rulesTable.addOne(errorToLoadable(error, fakeKey)));
				}
			})
			.createInterest();
	}

	async refecthRuleList(): Promise<void> {
		await this.rulesListInterest.invalidate({ owner: USER_RULE_LIST_KEY });
	}

	async createRule(params: CreateRuleParams): Promise<void> {
		try {
			await this.httpClient.post('rules', {
				body: toApiCreateRuleParams(params)
			});

			// After creating a rule, we can refetch the list to update the state
			await this.refecthRuleList();
		} catch (error) {
			console.error('Error creating rule:', error);
		}
	}
}
