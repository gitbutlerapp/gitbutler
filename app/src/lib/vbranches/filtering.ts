import type { RemoteCommit } from './types';

const FILTER_PROP_SEPARATOR = ':';
const FILTER_OR_VALUE_SEPARATOR = ',';

export enum FilterName {
	Author = 'author',
	Origin = 'origin'
}

enum FilterOriginValue {
	Local = 'local',
	Remote = 'remote'
}

export interface AppliedFilterInfo {
	name: FilterName;
	values: string[];
}

export interface AppliedFilter extends AppliedFilterInfo {
	id: string;
}

export interface FilterDescription {
	name: FilterName;
	allowedValues?: string[];
}

export interface FilterSuggestion {
	name: string;
	value?: string;
	description: string;
}

export const DEFAULT_FILTERS: FilterDescription[] = [
	{ name: FilterName.Author },
	{ name: FilterName.Origin, allowedValues: [FilterOriginValue.Local, FilterOriginValue.Remote] }
];

export const DEFAULT_FILTER_SUGGESTIONS: FilterSuggestion[] = [
	{ name: FilterName.Author, description: 'Filter by commit author' },
	{
		name: FilterName.Origin,
		value: FilterOriginValue.Local,
		description: 'Show only local commits'
	},
	{
		name: FilterName.Origin,
		value: FilterOriginValue.Remote,
		description: 'Show only upstream commits'
	}
];

export function commitMatchesFilter(
	commit: RemoteCommit,
	filter: AppliedFilter,
	isUpstream: boolean
): boolean {
	switch (filter.name) {
		case FilterName.Author:
			return !!commit.author.name && filter.values.includes(commit.author.name);
		case FilterName.Origin:
			return filter.values.includes(
				!isUpstream ? FilterOriginValue.Local : FilterOriginValue.Remote
			);
	}
}

export function parseFilterValues(
	value: string,
	filterDesc: FilterDescription
): string[] | undefined {
	const filterValue = value.replace(`${filterDesc.name}${FILTER_PROP_SEPARATOR}`, '');
	const listedValues = filterValue.split(FILTER_OR_VALUE_SEPARATOR);
	if (
		filterDesc.allowedValues === undefined ||
		listedValues.every((v) => filterDesc.allowedValues!.includes(v))
	) {
		return listedValues;
	}
	return undefined;
}

export function formatFilterValues(filter: AppliedFilter): string {
	return filter.values.join(FILTER_OR_VALUE_SEPARATOR);
}

export function formatFilterName(
	filter: AppliedFilter | FilterDescription | FilterSuggestion
): string {
	return `${filter.name}${FILTER_PROP_SEPARATOR}`;
}

function createAppliedFilterId(filterInfo: AppliedFilterInfo): string {
	return `${filterInfo.name}${FILTER_PROP_SEPARATOR}${filterInfo.values.sort().join(FILTER_OR_VALUE_SEPARATOR)}`;
}

export function createAppliedFilter(filterInfo: AppliedFilterInfo): AppliedFilter {
	return {
		...filterInfo,
		id: createAppliedFilterId(filterInfo)
	};
}

export function addAppliedFilter(
	filters: AppliedFilter[],
	toAdd: AppliedFilterInfo
): AppliedFilter[] {
	const newFilter = createAppliedFilter(toAdd);
	if (filters.some((filter) => filter.id === newFilter.id)) {
		return filters;
	}
	return [...filters, newFilter];
}

export function removeAppliedFilter(
	filters: AppliedFilter[],
	toRemove: AppliedFilter
): AppliedFilter[] {
	return filters.filter((filter) => filter.id !== toRemove.id);
}
