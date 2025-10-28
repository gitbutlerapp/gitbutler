import { registerInterest } from '$lib/interest/registerInterestFunction.svelte';
import { combine } from '$lib/network/loadable';
import { rulesTable } from '$lib/rules/rulesSlice';
import type { Loadable } from '$lib/network/types';
import type { AppRulesState } from '$lib/redux/store.svelte';
import type { RulesService } from '$lib/rules/rulesService';
import type { Rule } from '$lib/rules/types';
import type { Reactive } from '$lib/storeUtils';

export function getRulesList(
	appState: AppRulesState,
	rulesService: RulesService
): Reactive<Loadable<Rule[]>> {
	registerInterest(rulesService.getRuleListInterest());
	const current = $derived(rulesTable.selectors.selectAll(appState.rules));
	const currentCombined = $derived(combine(current));
	return {
		get current() {
			return currentCombined;
		}
	};
}
