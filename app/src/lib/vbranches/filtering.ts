import lscache from 'lscache';
import type { CommitMetrics, CommitStatus, RemoteCommit } from './types';

const FILTER_PROP_SEPARATOR = ':';
const FILTER_OR_VALUE_SEPARATOR = ',';
const APPLIED_FILTERS_CACHE_KEY = 'AppliedSearchFilters';
const APPLIED_FILTERS_CACHE_EXPIRATION_MIN = 7 * 24 * 60; // 7 days
const APPLIED_FILTERS_CACHE_MAX_SIZE = 5;

export enum FilterName {
	Author = 'author',
	Origin = 'origin',
	SHA = 'sha',
	File = 'file',
	Title = 'title',
	Body = 'body',
	Message = 'message'
}

type FormattedFilterName = `${FilterName}${typeof FILTER_PROP_SEPARATOR}`;

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

enum FilterSuggestionType {
	Static = 'static',
	Dynamic = 'dynamic'
}

interface FilterSuggestionBase {
	name: FilterName;
	value?: string;
}

export interface StaticFilterSuggestion extends FilterSuggestionBase {
	type: FilterSuggestionType.Static;
	value?: string;
	description: string;
}

export interface DynamicFilterSuggestion extends FilterSuggestionBase {
	type: FilterSuggestionType.Dynamic;
	value: string;
	metric: CommitMetrics;
}

export type FilterSuggestion = StaticFilterSuggestion | DynamicFilterSuggestion;

export interface FilterDescription {
	name: FilterName;
	allowedValues?: string[];
	suggestions?: StaticFilterSuggestion[];
	dynamicSuggestions?: DynamicFilterSuggestion[];
}

export const REMOTE_BRANCH_FILTERS: FilterDescription[] = [
	{
		name: FilterName.Author,
		suggestions: [
			{
				type: FilterSuggestionType.Static,
				name: FilterName.Author,
				description: 'Filter by commit author. Name must match exactly the given value'
			}
		]
	},
	{
		name: FilterName.SHA,
		suggestions: [
			{
				type: FilterSuggestionType.Static,
				name: FilterName.SHA,
				description: 'Filter by commit SHA. It must start with the given value'
			}
		]
	},
	{
		name: FilterName.File,
		suggestions: [
			{
				type: FilterSuggestionType.Static,
				name: FilterName.File,
				description: 'Filter by file path. It must include the given value'
			}
		]
	},
	{
		name: FilterName.Title,
		suggestions: [
			{
				type: FilterSuggestionType.Static,
				name: FilterName.Title,
				description: 'Filter by commit title. It must include the given value'
			}
		]
	},
	{
		name: FilterName.Body,
		suggestions: [
			{
				type: FilterSuggestionType.Static,
				name: FilterName.Body,
				description: 'Filter by commit body. It must include the given value'
			}
		]
	},
	{
		name: FilterName.Message,
		suggestions: [
			{
				type: FilterSuggestionType.Static,
				name: FilterName.Message,
				description: 'Filter by commit message. It must include the given value'
			}
		]
	}
];

const TRUNK_BRANCH_FILTERS: FilterDescription[] = [
	...REMOTE_BRANCH_FILTERS,
	{
		name: FilterName.Origin,
		allowedValues: [FilterOriginValue.Local, FilterOriginValue.Upstream],
		suggestions: [
			{
				type: FilterSuggestionType.Static,
				name: FilterName.Origin,
				value: FilterOriginValue.Local,
				description: 'Show only local commits'
			},
			{
				type: FilterSuggestionType.Static,
				name: FilterName.Origin,
				value: FilterOriginValue.Upstream,
				description: 'Show only upstream commits'
			}
		]
	}
];

export function getTrunkBranchFilters(
	suggestedValues: Partial<Record<FilterName, CommitMetrics[]>>
): FilterDescription[] {
	const result: FilterDescription[] = [];
	for (const filter of TRUNK_BRANCH_FILTERS) {
		const values = suggestedValues[filter.name];
		if (values === undefined) {
			result.push(filter);
			continue;
		}

		const f = { ...filter };
		f.dynamicSuggestions ??= [];
		for (const v of values) {
			f.dynamicSuggestions.push({
				type: FilterSuggestionType.Dynamic,
				name: f.name,
				value: v.name,
				metric: v
			});
		}
		result.push(f);
	}

	return result;
}

function commitMatchesFileFilter(commit: RemoteCommit, filter: AppliedFilter): boolean {
	if (!commit.filePaths) {
		return false;
	}

	for (const value of filter.values) {
		for (const filePath of commit.filePaths) {
			if (filePath.includes(value)) {
				return true;
			}
		}
	}
	return false;
}

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
				type === 'remote' ? FilterOriginValue.Upstream : FilterOriginValue.Local
			);
		case FilterName.SHA:
			return filter.values.some((sha) => commit.id.startsWith(sha));
		case FilterName.File:
			return commitMatchesFileFilter(commit, filter);
		case FilterName.Title:
			return filter.values.some((title) => commit.descriptionTitle?.includes(title));
		case FilterName.Body:
			return filter.values.some((body) => commit.descriptionBody?.includes(body));
		case FilterName.Message:
			return filter.values.some((message) => commit.description.includes(message));
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

export function tryToParseFilter(value: string): AppliedFilterInfo | undefined {
	const filterName = value.split(FILTER_PROP_SEPARATOR)[0];
	const filterDesc = TRUNK_BRANCH_FILTERS.find((desc) => desc.name === filterName);
	if (filterDesc === undefined) {
		return undefined;
	}

	const values = parseFilterValues(value, filterDesc);
	return values === undefined ? undefined : { name: filterDesc.name, values };
}

export function formatFilterValues(filter: AppliedFilter): string {
	return filter.values.join(FILTER_OR_VALUE_SEPARATOR);
}

export function formatFilterName(
	filter: AppliedFilter | FilterDescription | FilterSuggestion
): FormattedFilterName {
	return `${filter.name}${FILTER_PROP_SEPARATOR}`;
}

export function isFormattedFilterName(something: string): something is FormattedFilterName {
	const isFilterName = Object.values(FilterName).includes(
		something.split(FILTER_PROP_SEPARATOR)[0] as FilterName
	);

	return isFilterName && something.endsWith(FILTER_PROP_SEPARATOR);
}

export function getFilterName(formattedName: FormattedFilterName): FilterName {
	return formattedName.replace(FILTER_PROP_SEPARATOR, '') as FilterName;
}

export function createAppliedFilterId(filterInfo: AppliedFilterInfo): string {
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
	toAdd: AppliedFilterInfo,
	appendOnly?: boolean
): AppliedFilter[] {
	const newFilter = createAppliedFilter(toAdd);
	if (appendOnly) {
		return [...filters.filter((f) => f.id !== newFilter.id), newFilter];
	}
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
	return appliedFilters.some(
		(filter) =>
			suggestion.value &&
			filter.id === createAppliedFilterId({ name: suggestion.name, values: [suggestion.value] })
	);
}

export function getFilterEmoji(filterName: FilterName): string {
	switch (filterName) {
		case FilterName.Author:
			return '👤';
		case FilterName.Origin:
			return '🔗';
		case FilterName.SHA:
			return '🔑';
		case FilterName.File:
			return '📄';
		case FilterName.Title:
			return '🏷️';
		case FilterName.Body:
			return '📝';
		case FilterName.Message:
			return '💬';
	}
}

export function loadAppliedFilters(projectId: string, branchName: string): AppliedFilter[] {
	const filters = lscache.get(`${APPLIED_FILTERS_CACHE_KEY}:${projectId}:${branchName}`);
	return filters ? JSON.parse(filters) : [];
}

export function cacheAppliedFilters(
	projectId: string,
	branchName: string,
	filter: AppliedFilterInfo
): AppliedFilter[] {
	let filters = loadAppliedFilters(projectId, branchName);
	filters = addAppliedFilter(filters, filter, true).slice(-APPLIED_FILTERS_CACHE_MAX_SIZE);
	lscache.set(
		`${APPLIED_FILTERS_CACHE_KEY}:${projectId}:${branchName}`,
		JSON.stringify(filters),
		APPLIED_FILTERS_CACHE_EXPIRATION_MIN
	);
	return filters;
}

export function getSuggestionDescription(suggestion: FilterSuggestion): string {
	switch (suggestion.type) {
		case FilterSuggestionType.Static:
			return suggestion.description;
		case FilterSuggestionType.Dynamic:
			return `Found in ${suggestion.metric.commitIds.length} commits`;
	}
}
