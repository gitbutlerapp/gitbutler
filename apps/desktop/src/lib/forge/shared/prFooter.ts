import { isDefined } from '@gitbutler/ui/utils/typeguards';
import type { ForgePrService } from '$lib/forge/interface/forgePrService';
import type { ForgeReview } from '@gitbutler/core/api';

/**
 * Updates a pull request description with a table pointing to other pull
 * requests in the same stack.
 */
export async function updatePrDescriptionTables(
	projectId: string,
	prService: ForgePrService,
	prNumbers: number[]
) {
	if (prService && prNumbers.length > 1) {
		const prs = await Promise.all(prNumbers.map(async (id) => await prService.fetch(id)));
		const reviews: ForgeReview.ForgeReviewDescriptionUpdate[] = prs.filter(isDefined).map((pr) => ({
			number: BigInt(pr.number),
			body: pr.body ?? null,
			unitSymbol: prService.unit.symbol
		}));
		await prService.updateReviewFooters(projectId, reviews);
	}
}
