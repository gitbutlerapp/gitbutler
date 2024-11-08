import type { ForgePrService } from '../interface/forgePrService';

export const FOOTER_BOUNDARY_TOP = '<!-- GitButler Footer Boundary Top -->';
export const FOOTER_BOUNDARY_BOTTOM = '<!-- GitButler Footer Boundary Bottom -->';

/**
 * Updates a pull request description with a table pointing to other pull
 * requests in the same stack.
 */
export async function updatePrDescriptionTables(prService: ForgePrService, prNumbers: number[]) {
	if (prService && prNumbers.length > 1) {
		const prs = await Promise.all(prNumbers.map(async (id) => await prService.get(id)));
		const updates = prs.map((pr) => ({
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
	const head = (body?.split(FOOTER_BOUNDARY_TOP).at(0) || '').trim();
	const tail = (body?.split(FOOTER_BOUNDARY_BOTTOM).at(1) || '').trim();
	const footer = generateFooter(prNumber, allPrNumbers);
	const description = head + '\n\n' + footer + '\n\n' + tail;
	return description;
}

/**
 * Generates a footer for use in pull request descriptions when part of a stack.
 */
export function generateFooter(forPrNumber: number, allPrNumbers: number[]) {
	const stackIndex = allPrNumbers.findIndex((number) => number === forPrNumber);
	let footer = '';
	footer += FOOTER_BOUNDARY_TOP + '\n';
	footer += '---\n';
	footer += 'This is **part of a stack** made with GitButler:\n';
	allPrNumbers.forEach((prNumber, i) => {
		const current = i === stackIndex;
		footer += `- #${prNumber} ${current ? '👈 ' : ''}\n`;
	});
	footer += FOOTER_BOUNDARY_BOTTOM;
	return footer;
}
