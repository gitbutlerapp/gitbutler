import type { CommitStatus, RemoteCommit } from './types';

const FILTER_PROP_SEPARATOR = ':';
const FILTER_OR_VALUE_SEPARATOR = ',';

export enum FilterName {
	Author = 'author',
	Origin = 'origin',
	SHA = 'sha'
}

enum FilterOriginValue {
	Local = 'local',
	Upstream = 'upstream'
}

export interface AppliedFilterInfo {
	name: FilterName;
	values: string[];
}

export interface AppliedFilter extends AppliedFilterInfo {
	id: string;
}

export interface FilterSuggestion {
	name: string;
	value?: string;
	description: string;
}

export interface FilterDescription {
	name: FilterName;
	allowedValues?: string[];
	suggestions?: FilterSuggestion[];
}

export const REMOTE_BRANCH_FILTERS: FilterDescription[] = [
	{
		name: FilterName.Author,
		suggestions: [
			{
				name: FilterName.Author,
				description: 'Filter by commit author. Name must match exactly the given value'
			}
		]
	},
	{
		name: FilterName.SHA,
		suggestions: [
			{
				name: FilterName.SHA,
				description: 'Filter by commit SHA. It must start with the given value'
			}
		]
	}
];

export const TRUNK_BRANCH_FILTERS: FilterDescription[] = [
	...REMOTE_BRANCH_FILTERS,
	{
		name: FilterName.Origin,
		allowedValues: [FilterOriginValue.Local, FilterOriginValue.Upstream],
		suggestions: [
			{
				name: FilterName.Origin,
				value: FilterOriginValue.Local,
				description: 'Show only local commits'
			},
			{
				name: FilterName.Origin,
				value: FilterOriginValue.Upstream,
				description: 'Show only upstream commits'
			}
		]
	}
];

function commitMatchesFilter(
	commit: RemoteCommit,
	filter: AppliedFilter,
	type: CommitStatus
): boolean {
	switch (filter.name) {
		case FilterName.Author:
			return !!commit.author.name && filter.values.includes(commit.author.name);
		case FilterName.Origin:
			return filter.values.includes(
				type === 'upstream' ? FilterOriginValue.Upstream : FilterOriginValue.Local
			);
		case FilterName.SHA:
			return filter.values.some((sha) => commit.id.startsWith(sha));
	}
}

export function filterCommits(
	commits: RemoteCommit[],
	searchQuery: string | undefined,
	searchFilters: AppliedFilter[],
	type: CommitStatus
) {
	let filteredCommits = commits;
	for (const filter of searchFilters) {
		filteredCommits = filteredCommits.filter((commit) => commitMatchesFilter(commit, filter, type));
	}
	return searchQuery
		? filteredCommits.filter((commit) => commit.description.includes(searchQuery))
		: filteredCommits;
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

export function suggestionIsApplied(
	suggestion: FilterSuggestion,
	appliedFilters: AppliedFilter[]
): boolean {
	return appliedFilters.some((filter) => filter.name === suggestion.name);
}
