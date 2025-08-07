import { buildLoadableTable } from '$lib/redux/defaultSlices';
import type { LoadableBranchReviewListing } from '$lib/branches/types';

export const branchReviewListingTable =
	buildLoadableTable<LoadableBranchReviewListing>('branchReviewListing');
