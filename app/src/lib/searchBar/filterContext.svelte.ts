import {
	addAppliedFilter,
	removeAppliedFilter,
	type AppliedFilter,
	type AppliedFilterInfo
} from '$lib/vbranches/filtering';

let appliedFilters = $state<AppliedFilter[]>([]);
let searchQuery = $state<string | undefined>(undefined);

interface FilterContext {
	readonly appliedFilters: AppliedFilter[];
	searchQuery: string | undefined;
	addFilter: (filter: AppliedFilterInfo) => void;
	removeFilter: (filter: AppliedFilter) => void;
	popFilter: () => void;
	active: () => boolean;
	clear: () => void;
}

export function getFilterContext(): FilterContext {
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
		addFilter: (filter: AppliedFilterInfo) => {
			appliedFilters = addAppliedFilter(appliedFilters, filter);
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
		clear: () => {
			appliedFilters = [];
			searchQuery = undefined;
		}
	};
}
