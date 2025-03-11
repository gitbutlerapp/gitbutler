import type { ForgePrService } from '../interface/forgePrService';
import type { Branch } from '@gitbutler/shared/branches/types';
import type { PatchReview } from '@gitbutler/shared/patches/types';
import type { UserSimple } from '@gitbutler/shared/users/types';

export const STACKING_FOOTER_BOUNDARY_TOP = '<!-- GitButler Footer Boundary Top -->';
export const STACKING_FOOTER_BOUNDARY_BOTTOM = '<!-- GitButler Footer Boundary Bottom -->';

export const BUT_REVIEW_FOOTER_BOUNDARY_TOP = '<!-- GitButler Review Footer Boundary Top -->';
export const BUT_REVIEW_FOOTER_BOUNDARY_BOTTOM = '<!-- GitButler Review Footer Boundary Bottom -->';

export async function updateButRequestPrDescription(
	prService: ForgePrService,
	prNumber: number,
	butRequestUrl: string,
	butReview: Branch
) {
	const pr = await prService.get(prNumber);
	const prBody = pr.body || '\r\n';
	await prService.update(prNumber, {
		description: formatButRequestDescription(prBody, butRequestUrl, butReview)
	});
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
	return reviewAll.viewed
		.map((user: UserSimple) => `<img width="20" height="20" src="${user.avatarUrl}">`)
		.join(', ');
}

export function formatButRequestDescription(
	prBody: string,
	butRequestUrl: string,
	butReview: Branch
): string {
	const seriesSize = butReview.patches.length;
	const patches = butReview.patches
		.map(
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
	const descriptionLines = prDescription.split('\r\n');
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

	return `${before.join('\r\n')}\r\n${header}\r\n${injectable}\r\n${footer}\r\n${after.join('\r\n')}`;
}

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
