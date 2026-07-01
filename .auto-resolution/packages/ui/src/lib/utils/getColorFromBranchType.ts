import type { CellType } from "$components/commitLines/types";

const colorMap = {
	LocalOnly: "var(--commit-local)",
	LocalAndRemote: "var(--commit-remote)",
	Remote: "var(--commit-upstream)",
	Integrated: "var(--commit-integrated)",
	Error: "var(--fill-danger-bg)",
	Base: "var(--commit-upstream)",
};

export function getColorFromBranchType(type: CellType | "Error"): string {
	return colorMap[type];
}
