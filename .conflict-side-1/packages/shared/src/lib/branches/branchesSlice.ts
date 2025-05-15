import { buildLoadableTable } from '$lib/redux/defaultSlices';
import type { LoadableBranch } from '$lib/branches/types';

export const branchTable = buildLoadableTable<LoadableBranch>('branch');
