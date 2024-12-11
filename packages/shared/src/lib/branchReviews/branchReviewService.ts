import { upsertBranchReview, upsertBranchReviews } from '$lib/branchReviews/branchReviewsSlice';
import { upsertCommitReviews } from '$lib/branchReviews/commitReviewsSlice';
import {
	apiToBranchReview,
	apiToCommitReview,
	BranchReviewStatus,
	type ApiBranchReview,
	type BranchReview
} from '$lib/branchReviews/types';
import { InterestStore, type Interest } from '$lib/interest/intrestStore';
import { POLLING_REGULAR, POLLING_SLOW } from '$lib/polling';
import type { HttpClient } from '$lib/httpClient';
import type { AppDispatch } from '$lib/redux/store.svelte';

export class BranchReviewService {
	private readonly branchReviewsInterests = new InterestStore<{
		repositoryId: string;
		status: BranchReviewStatus;
	}>(POLLING_SLOW);
	private readonly branchReviewInterests = new InterestStore<{ branchId: string }>(POLLING_REGULAR);

	constructor(
		private readonly httpClient: HttpClient,
		private readonly appDispatch: AppDispatch
	) {}

	getBranchReviewsInterest(
		repositoryId: string,
		status: BranchReviewStatus = BranchReviewStatus.All
	): Interest {
		return this.branchReviewsInterests
			.findOrCreateSubscribable({ repositoryId, status }, async () => {
				const apiBranchReviews = await this.httpClient.get<ApiBranchReview[]>(
					`patch_stack/${repositoryId}?status=${status}`
				);

				const branchReviews = apiBranchReviews.map(apiToBranchReview);
				const commitReviews = apiBranchReviews
					.flatMap((apiBranchReview) => apiBranchReview.patches)
					.map(apiToCommitReview);

				this.appDispatch.dispatch(upsertBranchReviews(branchReviews));
				this.appDispatch.dispatch(upsertCommitReviews(commitReviews));
			})
			.createInterest();
	}

	getBranchReviewInterest(repositoryId: string, branchId: string): Interest {
		return this.branchReviewInterests
			.findOrCreateSubscribable({ branchId }, async () => {
				const apiBranchReview = await this.httpClient.get<ApiBranchReview>(
					`patch_stack/${repositoryId}/${branchId}`
				);
				const branchReview = apiToBranchReview(apiBranchReview);
				const commitReviews = apiBranchReview.patches.map(apiToCommitReview);

				this.appDispatch.dispatch(upsertBranchReviews([branchReview]));
				this.appDispatch.dispatch(upsertCommitReviews(commitReviews));
			})
			.createInterest();
	}

	async createBranchReview(
		repositoryId: string,
		branchId: string,
		oplogSha: string
	): Promise<BranchReview> {
		const apiBranchReview = await this.httpClient.post<ApiBranchReview>(
			`patch_stack/${repositoryId}`,
			{
				body: {
					branch_id: branchId,
					oplog_sha: oplogSha
				}
			}
		);
		const branchReview = apiToBranchReview(apiBranchReview);
		const commitReviews = apiBranchReview.patches.map(apiToCommitReview);

		this.appDispatch.dispatch(upsertBranchReview(branchReview));
		this.appDispatch.dispatch(upsertCommitReviews(commitReviews));

		return branchReview;
	}
}
