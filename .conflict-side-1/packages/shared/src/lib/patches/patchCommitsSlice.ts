import { buildLoadableTable } from '$lib/redux/defaultSlices';
import type { LoadablePatchCommit } from '$lib/patches/types';

export const patchCommitTable = buildLoadableTable<LoadablePatchCommit>('patchCommit');
