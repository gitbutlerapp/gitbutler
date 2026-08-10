/**
 * Natural name ordering shared by file lists: numbers compare numerically and
 * names that differ only by case retain their source order.
 */
const fileNameCollator = new Intl.Collator("en", {
	numeric: true,
	caseFirst: "lower",
	sensitivity: "base",
});

const compareFileNames = (a: string, b: string): number => fileNameCollator.compare(a, b);

/**
 * Compare complete file paths in the order a directory-first tree reveals
 * them. At each level directories precede files, then sibling names use the
 * natural ordering above.
 */
export const compareFilePaths = (a: string, b: string): number => {
	const aParts = a.split("/");
	const bParts = b.split("/");
	const sharedDepth = Math.min(aParts.length, bParts.length);

	for (let index = 0; index < sharedDepth; index++) {
		const aIsFile = index === aParts.length - 1;
		const bIsFile = index === bParts.length - 1;
		if (aIsFile !== bIsFile) return aIsFile ? 1 : -1;

		const aPart = aParts[index];
		const bPart = bParts[index];
		if (aPart === undefined || bPart === undefined) continue;

		const comparison = compareFileNames(aPart, bPart);
		if (comparison !== 0) return comparison;
	}

	return aParts.length - bParts.length;
};
