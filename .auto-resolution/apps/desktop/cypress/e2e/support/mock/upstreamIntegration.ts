import type { BranchStatusesResponse, IntegrationOutcome } from '$lib/upstream/types';

export const MOCK_BRANCH_STATUSES_RESPONSE: BranchStatusesResponse = {
	type: 'upToDate'
};

export const MOCK_INTEGRATION_OUTCOME: IntegrationOutcome = {
	deletedBranches: []
};
