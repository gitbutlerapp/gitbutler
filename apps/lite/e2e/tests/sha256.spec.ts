import { expect, test } from "../test.ts";

test.use({ objectFormat: "sha256", scenario: "project-in-single-branch-three-branch-stack.sh" });

const sha256 = /^[0-9a-f]{64}$/;

test("lists the stack of a sha256 repository", async ({ app, repo }) => {
	expect(repo.objectFormat).toBe("sha256");

	for (const commit of ["A: first commit", "B: first commit", "C: first commit"])
		await expect(app.commit(commit)).toBeVisible();
});

test("commits an uncommitted file with a sha256 id", async ({ app, repo }) => {
	await repo.write("added.txt");

	await app.commitFiles("Add a file on sha256", "added.txt");

	expect(repo.commitWithSubject("Add a file on sha256")).toMatch(sha256);
});

test("rewords a commit below the tip with a sha256 id", async ({ app, repo }) => {
	await app.rewordCommit("A: first commit", "A: reworded on sha256");

	await expect(app.commit("A: reworded on sha256")).toBeVisible();
	await expect(app.commit("C: first commit")).toBeVisible();
	expect(repo.commitWithSubject("A: reworded on sha256")).toMatch(sha256);
});
