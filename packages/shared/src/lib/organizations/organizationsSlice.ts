import { buildLoadableTable } from '$lib/redux/defaultSlices';
import type { LoadableOrganization } from '$lib/organizations/types';

export const organizationTable = buildLoadableTable<LoadableOrganization>('organization');
