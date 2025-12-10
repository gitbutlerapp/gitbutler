import { isDefined } from '@gitbutler/ui/utils/typeguards';
import type { ForgePrService } from '$lib/forge/interface/forgePrService';
import type { BranchDetails } from '$lib/stacks/stack';

export const STACKING_FOOTER_BOUNDARY_TOP = '<!-- GitButler Footer Boundary Top -->';
export const STACKING_FOOTER_BOUNDARY_BOTTOM = '<!-- GitButler Footer Boundary Bottom -->';

export const BUT_REVIEW_FOOTER_BOUNDARY_TOP = '<!-- GitButler Review Footer Boundary Top -->';
export const BUT_REVIEW_FOOTER_BOUNDARY_BOTTOM = '<!-- GitButler Review Footer Boundary Bottom -->';

export function unixifyNewlines(target: string): string {
	return target.split(/\r?\n/).join('\n');
}

/**
 * Updates a pull request description with a table pointing to other pull
 * requests in the same stack.
 */
export async function updatePrDescriptionTables(prService: ForgePrService, prNumbers: number[]) {
	if (prService && prNumbers.length > 1) {
		const prs = await Promise.all(prNumbers.map(async (id) => await prService.fetch(id)));
		const updates = prs.filter(isDefined).map((pr) => ({
			prNumber: pr.number,
			description: updateBody(pr.body, pr.number, prNumbers, prService.unit.symbol)
		}));
		await Promise.all(
			updates.map(async ({ prNumber, description }) => {
				await prService.update(prNumber, { description });
			})
		);
	}
}

type PrUpdate = {
	prNumber: number;
	targetBase: string;
	description: string;
};

export async function updateStackPrs(
	prService: ForgePrService,
	branchDetails: BranchDetails[],
	baseBranchName: string
) {
	if (branchDetails.length <= 1) return;
	const allPrNumbers = branchDetails.map((b) => b.prNumber).filter(isDefined);
	const updates: PrUpdate[] = [];
	let prevBranch: string | undefined = undefined;

	for (let i = branchDetails.length - 1; i >= 0; i--) {
		const details = branchDetails[i];
		if (!details) continue;
		const prNumber = details.prNumber;
		if (!isDefined(prNumber)) {
			prevBranch = details.name;
			continue;
		}
		const pr = await prService.fetch(prNumber);

		if (!isDefined(pr)) {
			prevBranch = details.name;
			continue;
		}

		updates.push({
			prNumber,
			description: updateBody(pr.body, pr.number, allPrNumbers, prService.unit.symbol),
			targetBase: prevBranch ?? baseBranchName
		});
		prevBranch = details.name;
	}

	if (updates.length > 0) {
		await Promise.all(
			updates.map(async ({ prNumber, targetBase, description }) => {
				await prService.update(prNumber, { description, targetBase });
			})
		);
	}
}

/**
 * Remove the PR description footer from the given PR numbers.
 */
export async function unstackPRs(
	prService: ForgePrService,
	prNumbers: number[],
	baseBranchName: string
) {
	if (prService && prNumbers.length > 0) {
		const prs = await Promise.all(prNumbers.map(async (id) => await prService.fetch(id)));
		const updates = prs.filter(isDefined).map((pr) => ({
			prNumber: pr.number,
			description: clearFooter(pr.body)
		}));

		await Promise.all(
			updates.map(async ({ prNumber, description }) => {
				await prService.update(prNumber, { description, targetBase: baseBranchName });
			})
		);
	}
}

/**
 * Replaces or inserts a new footer into an existing body of text.
 */
function updateBody(
	body: string | undefined,
	prNumber: number,
	allPrNumbers: number[],
	symbol: string
) {
	const head = (body?.split(STACKING_FOOTER_BOUNDARY_TOP).at(0) || '').trim();
	const tail = (body?.split(STACKING_FOOTER_BOUNDARY_BOTTOM).at(1) || '').trim();
	const footer = generateFooter(prNumber, allPrNumbers, symbol);
	const description = head + '\n\n' + footer + '\n\n' + tail;
	return description;
}

/**
 * Remove the footer from an existing body of text.
 */
function clearFooter(body: string | undefined) {
	if (!body) return body;
	if (!body.includes(STACKING_FOOTER_BOUNDARY_TOP)) return body;
	if (!body.includes(STACKING_FOOTER_BOUNDARY_BOTTOM)) return body;

	const head = (body?.split(STACKING_FOOTER_BOUNDARY_TOP).at(0) || '').trim();
	const tail = (body?.split(STACKING_FOOTER_BOUNDARY_BOTTOM).at(1) || '').trim();
	const description = head + '\n\n' + tail;
	return description;
}

/**
 * Generates a footer for use in pull request descriptions when part of a stack.
 */
function generateFooter(forPrNumber: number, allPrNumbers: number[], symbol: string) {
	const stackLength = allPrNumbers.length;
	const stackIndex = allPrNumbers.findIndex((number) => number === forPrNumber);
	const nth = stackLength - stackIndex;
	let footer = '';
	footer += STACKING_FOOTER_BOUNDARY_TOP + '\n';
	footer += '---\n';
	footer += `This is **part ${nth} of ${stackLength} in a stack** made with GitButler:\n`;
	allPrNumbers.forEach((prNumber, i) => {
		const current = i === stackIndex;
		footer += `- <kbd>&nbsp;${stackLength - i}&nbsp;</kbd> ${symbol}${prNumber} ${current ? '👈 ' : ''}\n`;
	});
	footer += STACKING_FOOTER_BOUNDARY_BOTTOM;
	return footer;
}
