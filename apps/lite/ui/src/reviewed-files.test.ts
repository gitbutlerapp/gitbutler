import { encodeBytes } from "#ui/api/bytes.ts";
import { branchFileParent, weakFileParentIdentityKey } from "#ui/addresses.ts";
import { moveBranchReviewedFiles, reviewedFilesQueryOptions } from "#ui/reviewed-files.ts";
import { QueryClient } from "@tanstack/react-query";
import { beforeEach, expect, test, vi } from "vitest";

const stored = new Map<string, unknown>();
vi.mock("idb-keyval", () => ({
	get: async (key: string) => stored.get(key),
	set: async (key: string, value: unknown) => void stored.set(key, value),
	del: async (key: string) => void stored.delete(key),
}));

beforeEach(() => stored.clear());

const contextId = (ref: string) =>
	weakFileParentIdentityKey(branchFileParent({ branchRef: encodeBytes(ref) }));

test("a view reading the new ref while reviewed files move gets the moved files", async () => {
	const client = new QueryClient({ defaultOptions: { queries: { staleTime: Infinity } } });
	const reviewed = new Map([["file.ts", new Set([1])]]);
	stored.set(`reviewed_files:v1:p1:${contextId("refs/heads/old")}`, reviewed);

	moveBranchReviewedFiles({
		queryClient: client,
		projectId: "p1",
		oldBranchRef: encodeBytes("refs/heads/old"),
		newBranchRef: encodeBytes("refs/heads/new"),
	});
	const moved = await client.fetchQuery(
		reviewedFilesQueryOptions("p1", contextId("refs/heads/new")),
	);

	expect(moved).toEqual(reviewed);
	expect([...stored.keys()]).toEqual([`reviewed_files:v1:p1:${contextId("refs/heads/new")}`]);
});
