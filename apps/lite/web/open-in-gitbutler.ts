/**
 * The menu item the harness panel adds (`harness/deepseek/client.js`): open the row a menu is for
 * in the desktop app, through its `but://` scheme, which the browser hands over.
 */
import type { NativeMenuPopupItem } from "#electron/ipc.ts";

export const OPEN_IN_GITBUTLER_ID = "but:open-in-gitbutler";

/** What a menu is for, as `native-menu.ts` passes it along; menus without one get no item. */
type Target = { path?: unknown; changeId?: unknown; commitId?: unknown; branchRef?: unknown };

const asTarget = (context: unknown): Target | null =>
	typeof context === "object" && context !== null ? (context as Target) : null;

const nonEmpty = (value: unknown): value is string => typeof value === "string" && value !== "";

export const withOpenInGitButler = (
	items: Array<NativeMenuPopupItem>,
	context: unknown,
): Array<NativeMenuPopupItem> => {
	const target = asTarget(context);
	const hasTarget =
		target !== null &&
		[target.path, target.changeId, target.commitId, target.branchRef].some(nonEmpty);
	return hasTarget
		? [
				...items,
				{ _tag: "Separator" },
				{ _tag: "Item", label: "Open in GitButler", itemId: OPEN_IN_GITBUTLER_ID },
			]
		: items;
};

/**
 * Open the app where the row lives: an uncommitted file in the uncommitted list, or a branch or
 * commit in the applied list, change id first as it survives amend and reword. The project is the
 * page's own, which is the app's when this page is served by a local but-server.
 */
export const openInGitButler = (context: unknown) => {
	const target = asTarget(context);
	const projectId = /^\/project\/([^/]+)/.exec(window.location.pathname)?.[1];
	if (target === null || projectId === undefined) return;
	const base = `but://app/project/${projectId}/workspace?`;
	const query = nonEmpty(target.path)
		? `active=uncommitted&uncommitted=${encodeURIComponent(target.path)}`
		: nonEmpty(target.changeId)
			? `applied=change:${encodeURIComponent(target.changeId)}`
			: nonEmpty(target.commitId)
				? `applied=commit:${encodeURIComponent(target.commitId)}`
				: nonEmpty(target.branchRef)
					? `applied=branch:${encodeURIComponent(target.branchRef)}`
					: null;
	if (query === null) return;
	const anchor = document.createElement("a");
	anchor.href = base + query;
	anchor.style.display = "none";
	document.body.append(anchor);
	anchor.click();
	anchor.remove();
};
