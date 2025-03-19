import { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';
import { InterestStore } from '@gitbutler/shared/interest/interestStore';
import { ProjectService } from '@gitbutler/shared/organizations/projectService';
import { POLLING_REGULAR } from '@gitbutler/shared/polling';
import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import type { Forge } from '$lib/forge/interface/forge';
import type { ForgePrService } from '$lib/forge/interface/forgePrService';
import type { Branch } from '@gitbutler/shared/branches/types';
import type { PatchReview } from '@gitbutler/shared/patches/types';
import type { Reactive } from '@gitbutler/shared/storeUtils';
import type { UserSimple } from '@gitbutler/shared/users/types';

export const STACKING_FOOTER_BOUNDARY_TOP = '<!-- GitButler Footer Boundary Top -->';
export const STACKING_FOOTER_BOUNDARY_BOTTOM = '<!-- GitButler Footer Boundary Bottom -->';

export const BUT_REVIEW_FOOTER_BOUNDARY_TOP = '<!-- GitButler Review Footer Boundary Top -->';
export const BUT_REVIEW_FOOTER_BOUNDARY_BOTTOM = '<!-- GitButler Review Footer Boundary Bottom -->';

export function unixifyNewlines(target: string): string {
	return target.split(/\r?\n/).join('\n');
}

export class BrToPrService {
	constructor(
		private readonly webRoutes: WebRoutesService,
		private readonly projectService: ProjectService,
		private readonly latestBranchLookupService: LatestBranchLookupService,
		private readonly forge: Reactive<Forge>
	) {}

	private readonly butRequestUpdateInterests = new InterestStore<{
		prNumber: number;
		branchId: string;
		repositoryId: string;
	}>(POLLING_REGULAR);

	refreshButRequestPrDescription(prNumber: number, branchId: string, repositoryId: string) {
		this.butRequestUpdateInterests.invalidate({ prNumber, branchId, repositoryId });
	}

	updateButRequestPrDescription(prNumber: number, branchId: string, repositoryId: string) {
		return this.butRequestUpdateInterests
			.findOrCreateSubscribable({ prNumber, branchId, repositoryId }, async () => {
				try {
					const prService = this.forge.current.prService;
					if (!prService) return;

					const project = await this.projectService.getProject(repositoryId);
					if (!project) return;

					const butReview = await this.latestBranchLookupService.getBranch(
						project.owner,
						project.slug,
						branchId
					);
					if (!butReview) return;

					const butlerRequestUrl = this.webRoutes.projectReviewBranchUrl({
						branchId,
						projectSlug: project.slug,
						ownerSlug: project.owner
					});

					// Then we can do a more accurate comparison of the latest body
					const prResult = await prService.fetch(prNumber);
					const pr = prResult.data;
					const prBody = unixifyNewlines(pr?.body || '\n');

					const newBody = unixifyNewlines(
						formatButRequestDescription(prBody, butlerRequestUrl, butReview)
					);

					if (prBody === newBody) return;

					await prService.update(prNumber, {
						description: newBody
					});
				} catch (error: unknown) {
					// This is not an essential function so we can let it
					// quietly fail
					console.error(error);
				}
			})
			.createInterest();
	}
}

function reviewStatusToIcon(status: string) {
	if (status === 'approved') {
		return '✅';
	} else if (status === 'in-discussion') {
		return '💬';
	} else if (status === 'changes-requested') {
		return '⚠️';
	}
	return '⏳';
}

function reviewAllToAvatars(reviewAll: PatchReview) {
	return [...reviewAll.signedOff, ...reviewAll.rejected]
		.map((user: UserSimple) => `<img width="20" height="20" src="${user.avatarUrl}">`)
		.join(' ');
}

export function formatButRequestDescription(
	prBody: string,
	butRequestUrl: string,
	butReview: Branch
): string {
	const seriesSize = butReview.patches?.length || 0;
	const patches = butReview.patches
		?.map(
			(patch) =>
				`| ${seriesSize - (patch.position || 0)}/${seriesSize} | [${patch.title}](${butRequestUrl}/commit/${patch.changeId}) | ${reviewStatusToIcon(patch.reviewStatus)} | ${reviewAllToAvatars(patch.reviewAll)} |`
		)
		.join('\n');

	let summary = `**${butReview.title}**\n`;
	if (butReview.description) {
		summary += `\n\n${butReview.description}`;
	}

	const description = `---
⧓ Review in [Butler Review \`#${butReview.branchId}\`](${butRequestUrl})

${summary}

${seriesSize} commit series (version ${butReview.version || 1})

| Series | Commit Title | Status | Reviewers | 
| --- | --- | --- | --- |
${patches}

_Please leave review feedback in the [Butler Review](${butRequestUrl})_`;

	const newPrDescription = upsertDescription(
		BUT_REVIEW_FOOTER_BOUNDARY_TOP,
		BUT_REVIEW_FOOTER_BOUNDARY_BOTTOM,
		prBody,
		description
	);
	return newPrDescription;
}

function upsertDescription(
	header: string,
	footer: string,
	prDescription: string,
	injectable: string
): string {
	const descriptionLines = prDescription.split('\n');
	const before = [];
	const after = [];

	let headerDetected = false;
	let footerDetected = false;

	for (const line of descriptionLines) {
		if (!headerDetected) {
			if (line.startsWith(header)) {
				headerDetected = true;
				continue;
			}

			before.push(line);
		}

		if (!footerDetected) {
			if (line.startsWith(footer)) {
				footerDetected = true;
				continue;
			}
		}

		if (footerDetected) {
			after.push(line);
		}
	}

	return `${before.join('\n')}\n${header}\n${injectable}\n${footer}\n${after.join('\n')}`;
}

/**
 * Updates a pull request description with a table pointing to other pull
 * requests in the same stack.
 */
export async function updatePrDescriptionTables(prService: ForgePrService, prNumbers: number[]) {
	if (prService && prNumbers.length > 1) {
		const prs = await Promise.all(prNumbers.map(async (id) => (await prService.fetch(id)).data));
		const updates = prs.filter(isDefined).map((pr) => ({
			prNumber: pr.number,
			description: updateBody(pr.body, pr.number, prNumbers)
		}));
		await Promise.all(
			updates.map(async ({ prNumber, description }) => {
				await prService.update(prNumber, { description });
			})
		);
	}
}

/**
 * Replaces or inserts a new footer into an existing body of text.
 */
function updateBody(body: string | undefined, prNumber: number, allPrNumbers: number[]) {
	const head = (body?.split(STACKING_FOOTER_BOUNDARY_TOP).at(0) || '').trim();
	const tail = (body?.split(STACKING_FOOTER_BOUNDARY_BOTTOM).at(1) || '').trim();
	const footer = generateFooter(prNumber, allPrNumbers);
	const description = head + '\n\n' + footer + '\n\n' + tail;
	return description;
}

/**
 * Generates a footer for use in pull request descriptions when part of a stack.
 */
export function generateFooter(forPrNumber: number, allPrNumbers: number[]) {
	const stackLength = allPrNumbers.length;
	const stackIndex = allPrNumbers.findIndex((number) => number === forPrNumber);
	const nth = stackLength - stackIndex;
	let footer = '';
	footer += STACKING_FOOTER_BOUNDARY_TOP + '\n';
	footer += '---\n';
	footer += `This is **part ${nth} of ${stackLength} in a stack** made with GitButler:\n`;
	allPrNumbers.forEach((prNumber, i) => {
		const current = i === stackIndex;
		footer += `- <kbd>&nbsp;${stackLength - i}&nbsp;</kbd> #${prNumber} ${current ? '👈 ' : ''}\n`;
	});
	footer += STACKING_FOOTER_BOUNDARY_BOTTOM;
	return footer;
}
