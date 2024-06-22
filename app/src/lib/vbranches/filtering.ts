import type { RemoteCommit } from './types';

export enum FilterName {
	Author = 'author',
	Origin = 'origin'
}

enum FilterOriginValue {
	Local = 'local',
	Remote = 'remote'
}

export interface AppliedFilter {
	name: FilterName;
	values: string[];
}

export interface FilterDescription {
	name: FilterName;
	allowedValues?: string[];
}

export const DEFAULT_FILTERS: FilterDescription[] = [
	{ name: FilterName.Author },
	{ name: FilterName.Origin, allowedValues: [FilterOriginValue.Local, FilterOriginValue.Remote] }
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
