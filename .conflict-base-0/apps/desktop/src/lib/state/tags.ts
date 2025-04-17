export enum ReduxTag {
	Diff = 'Diff',
	Stacks = 'Stacks',
	StackDetails = 'StackDetails',
	WorktreeChanges = 'WorktreeChanges',
	CommitChanges = 'CommitChanges',
	BranchChanges = 'BranchChanges',
	PullRequests = 'PullRequests',
	GitLabPullRequests = 'GitLabPullRequests',
	Checks = 'Checks',
	RepoInfo = 'RepoInfo',
	BaseBranchData = 'BaseBranchData',
	UpstreamIntegrationStatus = 'UpstreamIntegrationStatus',
	BranchListing = 'BranchListing'
}

type Tag<T extends string | number> = {
	type: ReduxTag;
	id: T;
};

const LIST = 'LIST';

// We always want to provide either, just the list or the list and the item.
// This means that we can either invalidate all of them, or an individual item.

export function providesList(tag: ReduxTag): Tag<typeof LIST> {
	return { type: tag, id: LIST };
}

export function providesItem<T extends string | number>(
	tag: ReduxTag,
	id: T
): [Tag<T>, Tag<typeof LIST>] {
	return [
		{ type: tag, id },
		{ type: tag, id: LIST }
	];
}

export function providesItems<T extends string | number>(
	tag: ReduxTag,
	ids: T[]
): Tag<T | typeof LIST>[] {
	const itemTags = ids.map((id) => ({ type: tag, id }));
	return [...itemTags, { type: tag, id: LIST }];
}

export function invalidatesList(tag: ReduxTag): Tag<typeof LIST> {
	return { type: tag, id: LIST };
}

export function invalidatesItem<
	T extends string | number | undefined,
	OutTag = Tag<T extends undefined ? typeof LIST : T>
>(tag: ReduxTag, id: T): OutTag {
	if (id === undefined) {
		return { type: tag, id: LIST } as OutTag;
	}
	return { type: tag, id } as OutTag;
}
