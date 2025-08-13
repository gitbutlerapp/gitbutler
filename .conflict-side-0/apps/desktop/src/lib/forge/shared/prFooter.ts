import { isDefined } from '@gitbutler/ui/utils/typeguards';
import type { ForgePrService } from '$lib/forge/interface/forgePrService';

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
