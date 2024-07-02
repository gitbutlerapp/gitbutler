import { Project } from '$lib/backend/projects';
import { getContext } from '$lib/utils/context';
import {
	addAppliedFilter,
	cacheAppliedFilters,
	createAppliedFilterId,
	loadAppliedFilters,
	removeAppliedFilter,
	type AppliedFilter,
	type AppliedFilterInfo,
	type FilterSuggestion
} from '$lib/vbranches/filtering';

let activeBranch = $state<string | undefined>(undefined);
let appliedFilters = $state<AppliedFilter[]>([]);
let searchQuery = $state<string | undefined>(undefined);
let recentFilters = $state<AppliedFilter[]>([]);

interface FilterContext {
	readonly appliedFilters: AppliedFilter[];
	searchQuery: string | undefined;
	readonly recentFilters: AppliedFilter[];
	hasRecentFilter: (suugestion: FilterSuggestion) => boolean;
	addFilter: (filter: AppliedFilterInfo) => void;
	removeFilter: (filter: AppliedFilter) => void;
	popFilter: () => void;
	active: () => boolean;
	init: (branchName: string) => void;
	clear: () => void;
}

export function getFilterContext(): FilterContext {
	const project = getContext(Project);

	return {
		get appliedFilters() {
			return appliedFilters;
		},
		get searchQuery() {
			return searchQuery;
		},
		set searchQuery(value: string | undefined) {
			searchQuery = value;
		},
		get recentFilters() {
			return [...recentFilters].reverse();
		},
		hasRecentFilter: (suggestion: FilterSuggestion) => {
			if (!suggestion.value) {
				return false;
			}
			return recentFilters.some(
				(filter) =>
					filter.id ===
					createAppliedFilterId({ name: suggestion.name, values: [suggestion.value!] })
			);
		},
		addFilter: (filter: AppliedFilterInfo) => {
			appliedFilters = addAppliedFilter(appliedFilters, filter);

			if (activeBranch) {
				recentFilters = cacheAppliedFilters(project.id, activeBranch, filter);
			}
		},
		removeFilter: (filter: AppliedFilter) => {
			appliedFilters = removeAppliedFilter(appliedFilters, filter);
		},
		popFilter: () => {
			if (appliedFilters.length === 0) {
				return;
			}
			appliedFilters.pop();
		},
		active: () => {
			return appliedFilters.length > 0 || !!searchQuery;
		},
		init: (branchName: string) => {
			activeBranch = branchName;
			appliedFilters = [];
			searchQuery = undefined;
			recentFilters = loadAppliedFilters(project.id, branchName);
		},
		clear: () => {
			appliedFilters = [];
			searchQuery = undefined;
		}
	};
}
