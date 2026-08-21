import { decodeBytes } from "#ui/api/bytes.ts";
import type { CursorItem, CursorName } from "#ui/cursors.ts";
import type { PageId } from "#ui/projects/project.ts";
import type { Address } from "#ui/addresses.ts";

/**
 * The workspace URL's query params: where the user is, in one flat namespace.
 * Every param is a plain string with a total decoder, so a corrupt or stale
 * URL degrades to defaults rather than erroring, and defaults are left out
 * rather than written.
 *
 * There is intentionally no URL query parameter for the diff cursor: its exact
 * visual line range stays in Redux instead.
 */
export type UrlQueryParams = {
	page?: Exclude<PageId, "workspace">;
	active?: "uncommitted";
	applied?: string;
	uncommitted?: string;
	unapplied?: string;
	upstream?: string;
	files?: string;
};

/** The lists whose cursor is a URL param — every list but `diff`. */
export type UrlCursorName = Exclude<CursorName, "diff">;

export const isUrlCursor = (list: CursorName): list is UrlCursorName => list !== "diff";

/**
 * Address codec: `branch:<full-ref>`, `change:<change-id>` (first choice — a
 * change id survives amend and reword, so the URL needs no repair) and
 * `commit:<commit-id>` only for a commit that has no change id. Other addresses
 * are not addressable places.
 */
const encodeAddress = (address: Address): string | null => {
	switch (address._tag) {
		case "Branch":
			return `branch:${decodeBytes(address.branchRef)}`;
		case "Commit":
			return address.changeId !== "" ? `change:${address.changeId}` : `commit:${address.commitId}`;
		default:
			return null;
	}
};

const branchPrefix = "branch:";
const changePrefix = "change:";
const commitPrefix = "commit:";

const encodePath = (path: string): string => path;

const cursorParam: { [L in UrlCursorName]: (item: CursorItem[L]) => string | null } = {
	applied: encodeAddress,
	unapplied: encodeAddress,
	upstream: encodeAddress,
	uncommitted: encodePath,
	files: encodePath,
};

/** The full ref name a cursor param carries, if it names a branch. */
export const branchParamRef = (param: string | undefined): string | null =>
	param !== undefined && param.startsWith(branchPrefix) ? param.slice(branchPrefix.length) : null;

/** The commit reference a cursor param carries, if it names a commit. */
export const commitParamRef = (
	param: string | undefined,
): { changeId: string } | { commitId: string } | null =>
	param === undefined
		? null
		: param.startsWith(changePrefix)
			? { changeId: param.slice(changePrefix.length) }
			: param.startsWith(commitPrefix)
				? { commitId: param.slice(commitPrefix.length) }
				: null;

export const encodeCursorParam = <L extends UrlCursorName>(
	list: L,
	item: CursorItem[L],
): string | null => cursorParam[list](item);
