/**
 * Shared reactive state for per-file context menus in diff view components.
 *
 * Both MultiDiffView and FloatingDiffModal render a list of file diffs, each
 * with its own context menu, trigger element, kebab button, and open/closed
 * state. This class centralises those four records so the pattern is not
 * duplicated across components.
 */
import type { TreeChange } from "$lib/hunks/change";

export type ContextMenuLike = {
	open: (trigger: HTMLElement | MouseEvent, opts: { changes: TreeChange[] }) => void;
};

export class FileContextMenuState<TMenu extends ContextMenuLike = ContextMenuLike> {
	/** Component instances, keyed by file path. */
	contextMenus = $state<Record<string, TMenu>>({});
	/** Header / row elements used as right-click triggers, keyed by file path. */
	headerTriggers = $state<Record<string, HTMLElement>>({});
	/** Kebab button elements used as left-click triggers, keyed by file path. */
	buttonElements = $state<Record<string, HTMLElement>>({});
	/** Whether each file's context menu is currently open, keyed by file path. */
	menuOpenStates = $state<Record<string, boolean>>({});
}
