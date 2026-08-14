import { decodeBytes } from "#ui/api/bytes.ts";
import type { ListItem, ListName } from "#ui/cursors.ts";
import type { OutlineTab } from "#ui/projects/project.ts";
import type { Operand } from "#ui/operands.ts";

/**
 * The workspace URL's query params: where the user is, in one flat namespace.
 * Every param is a plain string with a total decoder, so a corrupt or stale
 * URL degrades to defaults rather than erroring, and defaults are left out
 * rather than written.
 *
 * The diff cursor has no param: its identity is the exact selected line
 * groups, which no legible string carries, so it stays in the store.
 */
export type UrlQueryParams = {
	page?: Exclude<OutlineTab, "workspace">;
	list?: "uncommitted";
	stacks?: string;
	uncommitted?: string;
	branches?: string;
	upstream?: string;
	files?: string;
};

/** The lists whose cursor is a URL param — every list but `diff`. */
export type UrlListName = Exclude<ListName, "diff">;

export const isUrlList = (list: ListName): list is UrlListName => list !== "diff";

/**
 * Operand codec: `branch:<full-ref>`, `change:<change-id>` (first choice — a
 * change id survives amend and reword, so the URL needs no repair) and
 * `commit:<commit-id>` only for a commit that has no change id. Other operands
 * are not addressable places.
 */
const encodeOperand = (operand: Operand): string | null => {
	switch (operand._tag) {
		case "Branch":
			return `branch:${decodeBytes(operand.branchRef)}`;
		case "Commit":
			return operand.changeId !== "" ? `change:${operand.changeId}` : `commit:${operand.commitId}`;
		default:
			return null;
	}
};

const branchPrefix = "branch:";
const changePrefix = "change:";
const commitPrefix = "commit:";

const encodePath = (path: string): string => path;

const cursorParam: { [L in UrlListName]: (item: ListItem[L]) => string | null } = {
	stacks: encodeOperand,
	branches: encodeOperand,
	upstream: encodeOperand,
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

export const encodeCursorParam = <L extends UrlListName>(
	list: L,
	item: ListItem[L],
): string | null => cursorParam[list](item);
