import { openWorkspace } from "../src/setup.ts";
import { test } from "../src/test.ts";
import { commitRow, getByTestId, stack } from "../src/util.ts";
import { expect } from "@playwright/test";

/**
 * Follow-up coverage for PR #13904 (`head_info` migration).
 *
 * The shared setup (`project-with-deleted-stack-ref.sh`) leaves the
 * workspace with one stack whose `head_info` payload has:
 *
 *   * `Stack.id === null`
 *   * `Stack.segments[0].refName === null` (anonymous segment)
 *
 * The current `headInfoAdapters` pipeline has known gaps on both axes
 * (collision-prone `stackKey` fallback, branch-name lookups that skip
 * anonymous segments, `stackById(null)` paths). These two tests assert
 * the UI behaviour we'd want once those gaps are closed; they are
 * expected to FAIL today and serve as a baseline.
 */

test("renders a workspace stack whose tip ref has been deleted (anon segment)", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-with-deleted-stack-ref.sh");
	await openWorkspace(page);

	// The stack is still reachable from the workspace commit, so the
	// workspace view should surface it — even though the segment lost its
	// branch ref.
	await expect(stack(page)).toHaveCount(1);
	await expect(commitRow(page, "feature: extend b_file")).toBeVisible();
	await expect(commitRow(page, "feature: add b_file")).toBeVisible();
	await expect(commitRow(page)).toHaveCount(2);

	// `virtual_branches.toml` still records this segment as `feature`, and
	// `but-graph`'s reconciliation re-attaches that name to the stack
	// metadata even after the ref is deleted. A fix should surface that
	// recovered name in the branch header rather than rendering an empty
	// label or the base SHA. Today this assertion fails because anonymous
	// segments fall through `selectWorkspaceStackDetails`' branch-name
	// lookup and the header has no text to render.
	const featureHeader = getByTestId(page, "branch-header").filter({ hasText: "feature" });
	await expect(featureHeader).toBeVisible();
});

test("can interact with a workspace stack that has a null id", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-deleted-stack-ref.sh");
	await openWorkspace(page);

	// `Stack.id === null` makes anything that keys off the stack id (e.g.
	// `stackById`, `allLocalCommits`) drop the stack from its derived
	// view. Clicking a commit row exercises a selection path that calls
	// back into those lookups — today this is likely to either fail to
	// open the drawer or open it against an empty selection.
	const firstCommit = commitRow(page, "feature: extend b_file");
	await expect(firstCommit).toBeVisible();
	await firstCommit.click();

	// The commit details drawer should open and reflect the clicked commit.
	const drawer = getByTestId(page, "commit-drawer");
	await expect(drawer).toBeVisible();
	await expect(drawer).toContainText("feature: extend b_file");
});
