import { buildLoadableTable } from '$lib/redux/defaultSlices';
import type { LoadableBranchUuid } from '$lib/branches/types';

export const latestBranchLookupTable = buildLoadableTable<LoadableBranchUuid>('latestBranchLookup');
