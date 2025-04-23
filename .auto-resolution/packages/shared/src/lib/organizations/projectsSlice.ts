import { buildLoadableTable } from '$lib/redux/defaultSlices';
import type { LoadableProject } from '$lib/organizations/types';

export const projectTable = buildLoadableTable<LoadableProject>('project');
