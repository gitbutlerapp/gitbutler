import { buildLoadableTable } from '$lib/redux/defaultSlices';
import type { LoadablePatchIdable } from '$lib/patches/types';

export const patchIdableTable = buildLoadableTable<LoadablePatchIdable>('patchIdable');
