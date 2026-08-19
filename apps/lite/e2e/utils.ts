import { execFileSync } from "node:child_process";
import { expect } from "@playwright/test";

export const assertHeadBranch = (repositoryPath: string, branch: string): void => {
	const head = execFileSync("git", ["-C", repositoryPath, "symbolic-ref", "--short", "HEAD"], {
		encoding: "utf8",
	}).trim();
	expect(head).toBe(branch);
};
