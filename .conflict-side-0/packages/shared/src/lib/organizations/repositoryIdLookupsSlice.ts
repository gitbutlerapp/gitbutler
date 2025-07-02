import { buildLoadableTable } from '$lib/redux/defaultSlices';
import type { LoadableRepositoryId } from '$lib/organizations/types';

export const repositoryIdLookupTable =
	buildLoadableTable<LoadableRepositoryId>('repositoryIdLookup');
