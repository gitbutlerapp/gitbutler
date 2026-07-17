import { isDefined } from "@gitbutler/ui/utils/typeguards";
import type { PrService } from "$lib/forge/prService.svelte";
import type { ForgeReviewUpdate, Segment } from "@gitbutler/but-sdk";

export const STACKING_FOOTER_BOUNDARY_TOP = "<!-- GitButler Footer Boundary Top -->";
export const STACKING_FOOTER_BOUNDARY_BOTTOM = "<!-- GitButler Footer Boundary Bottom -->";

export const BUT_REVIEW_FOOTER_BOUNDARY_TOP = "<!-- GitButler Review Footer Boundary Top -->";
export const BUT_REVIEW_FOOTER_BOUNDARY_BOTTOM = "<!-- GitButler Review Footer Boundary Bottom -->";

export function unixifyNewlines(target: string): string {
	return target.split(/\r?\n/).join("\n");
}

/**
 * Sync stack info onto the pull requests of a stack: description footers, or
 * GitHub's native stacks where the repo has them enabled (decided backend-side).
 *
 * `prNumbers` are expected top-first, as the stack segments are ordered.
 */
export async function updatePrStackInfo(
	prService: PrService,
	projectId: string,
	prNumbers: number[],
	unitSymbol = "#",
) {
	if (prNumbers.length <= 1) return;
	// The backend expects a single stack ordered bottom-to-top.
	const bottomToTop = [...prNumbers].reverse();
	const prs = await Promise.all(
		bottomToTop.map(async (id) => await prService.fetch(projectId, id)),
	);
	const updates: ForgeReviewUpdate[] = prs.filter(isDefined).map((pr) => ({
		number: pr.number,
		body: pr.body ?? null,
		unitSymbol,
		targetBranch: null,
	}));
	await prService.updateFooters(projectId, updates);
}

/**
 * Sync stack info and target branches after the stack structure changed
 * (e.g. branches were reordered). `branchDetails` are expected top-first.
 */
export async function updateStackPrs(
	prService: PrService,
	projectId: string,
	branchDetails: Segment[],
	baseBranchName: string,
	unitSymbol = "#",
) {
	if (branchDetails.length <= 1) return;
	const updates: ForgeReviewUpdate[] = [];
	let prevBranch: string | undefined = undefined;

	// Walk bottom-to-top, chaining each PR onto the branch below it.
	for (let i = branchDetails.length - 1; i >= 0; i--) {
		const details = branchDetails[i];
		if (!details) continue;
		const branchName = details.refName?.displayName;
		if (!branchName) continue;
		const prNumber = details.metadata?.review.pullRequest;
		if (!isDefined(prNumber)) {
			prevBranch = branchName;
			continue;
		}
		const pr = await prService.fetch(projectId, prNumber);

		if (!isDefined(pr)) {
			prevBranch = branchName;
			continue;
		}

		updates.push({
			number: prNumber,
			body: pr.body ?? null,
			unitSymbol,
			targetBranch: prevBranch ?? baseBranchName,
		});
		prevBranch = branchName;
	}

	if (updates.length > 0) {
		await prService.updateFooters(projectId, updates);
	}
}

/**
 * Remove the PR description footer from the given PR numbers.
 */
export async function unstackPRs(
	prService: PrService,
	projectId: string,
	prNumbers: number[],
	baseBranchName: string,
) {
	if (prService && prNumbers.length > 0) {
		const prs = await Promise.all(
			prNumbers.map(async (id) => await prService.fetch(projectId, id)),
		);
		const updates = prs.filter(isDefined).map((pr) => ({
			prNumber: pr.number,
			description: clearFooter(pr.body),
		}));

		await Promise.all(
			updates.map(async ({ prNumber, description }) => {
				await prService.update(projectId, prNumber, {
					description,
					targetBase: baseBranchName,
				});
			}),
		);
	}
}

/**
 * Remove the footer from an existing body of text.
 */
function clearFooter(body: string | undefined) {
	if (!body) return body;
	if (!body.includes(STACKING_FOOTER_BOUNDARY_TOP)) return body;
	if (!body.includes(STACKING_FOOTER_BOUNDARY_BOTTOM)) return body;

	const head = (body?.split(STACKING_FOOTER_BOUNDARY_TOP).at(0) || "").trim();
	const tail = (body?.split(STACKING_FOOTER_BOUNDARY_BOTTOM).at(1) || "").trim();
	const description = head + "\n\n" + tail;
	return description;
}
