import type { CodeViewOptions } from "@pierre/diffs";
import { createElement, type ReactElement, useLayoutEffect, useRef } from "react";
import {
	hunkAddress,
	hunkAddressContainsLine,
	addressIdentityKey,
	type HunkAddress,
	type Address,
} from "#ui/addresses.ts";
import { assert } from "#ui/assert.ts";
import { icons } from "#ui/components/icons.ts";
import { getOperationSources } from "#ui/operations/pending-operation.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppStore } from "#ui/store.ts";
import {
	DiffGutterPortals,
	type GutterCheckboxGroup,
	type GutterStore,
	type GutterTarget,
} from "./DiffGutterPortals.tsx";
import { diffLineTargetFromElement, type DiffLineTarget } from "./diff-line-target.ts";

const GUTTER_SLOT_ATTRIBUTE = "data-gitbutler-diff-gutter-slot";
const GUTTER_SLOT_KIND_ATTRIBUTE = "data-gitbutler-diff-gutter-slot-kind";
const GUTTER_DRAG_HANDLE_ATTRIBUTE = "data-hunk-drag-handle";
const GUTTER_GROUP_ATTRIBUTE = "data-gitbutler-diff-gutter-group";
const GUTTER_HOVERED_ATTRIBUTE = "data-gitbutler-diff-gutter-hovered";
const ACTIONS_ATTRIBUTE = "data-gitbutler-diff-actions";
const ACTIONS_FILLED_ATTRIBUTE = "data-gitbutler-diff-actions-filled";
const ACTIONS_DRAGGABLE_ATTRIBUTE = "data-gitbutler-diff-actions-draggable";
const DRAG_PREVIEW_ATTRIBUTE = "data-gitbutler-diff-drag-preview";
const HUNK_BAND_ATTRIBUTE = "data-gitbutler-diff-hunk-band";
const HUNK_BAND_CHECKED_ATTRIBUTE = "data-checked";
const COMMENT_SLOT_ATTRIBUTE = "data-gitbutler-diff-comment-slot";
const OPERATION_SOURCE_ATTRIBUTE = "data-gitbutler-operation-source";
const CHECK_DRAG_ATTRIBUTE = "data-gitbutler-diff-check-drag";

export const diffGutterUnsafeCSS = `
	/* While a press is painting checkboxes, the lines it crosses are the gesture's, not text. */
	:host([${CHECK_DRAG_ATTRIBUTE}]) {
		user-select: none;
	}

	:host {
		--gitbutler-diff-gutter-control-width: 1lh;
		/* Two columns of the same width, the hunk's and the line's, the first flush with the panel's
		   edge so nothing shows between the two. A panel's resize handle expands its 1px separator to
		   a 10px hit target that reaches ~4.5px in, so that sliver of the hunk column answers the
		   resize first; the rest of it is the hunk's own. */
		--gitbutler-diff-gutter-width: calc(2 * var(--gitbutler-diff-gutter-control-width));
		/* The break between the gutter and the change bar, as wide as the one the numbers already
		   keep from the code. It cannot be left unpainted — the line's fill is one box, and the
		   controls sit over it — so it is painted in the surface the other break reveals, which is
		   Pierre's own and not the app's, since the two part ways in the dark theme. */
		--gitbutler-diff-gutter-seam: 2px;
		/* The inset the card keeps around the controls it carries. */
		--gitbutler-diff-actions-padding: 2px;
		--gitbutler-diff-gutter-seam-color: var(--diffs-background, var(--bg-1));
	}

	[data-column-number] {
		/* Where the gutter's business ends and the file's own margin begins. */
		--gitbutler-diff-number-start: var(--gitbutler-diff-gutter-width);
		--gitbutler-diff-number-film: transparent;

		padding-left: calc(2ch + var(--gitbutler-diff-number-start));
		background-image: linear-gradient(
			to right,
			transparent var(--gitbutler-diff-number-start),
			var(--gitbutler-diff-number-film) var(--gitbutler-diff-number-start)
		);
	}

	/* A press on the numbers selects lines, so they take a film under the pointer to say so. It is
	   mixed from the line's own foreground, the way the checkboxes are, so a changed line answers in
	   its own colour rather than in ink. The gutter has its own answers, and a pointer resting on
	   one of them is not asking for this. */
	[data-column-number]:hover:not(
			:has(
				> slot[${GUTTER_SLOT_ATTRIBUTE}]:hover,
				> [${HUNK_BAND_ATTRIBUTE}]:hover,
				> [${ACTIONS_ATTRIBUTE}]:hover
			)
		) {
		--gitbutler-diff-number-film: color-mix(in srgb, currentColor 10%, transparent);
	}

	/* The gutter ends where the file's own margin begins, so the line's fill stops short of the
	   change bar and picks up again past it — the same seam the numbers keep from the code. The
	   controls sit above this, and the fill is the row's, so only a stripe of the surface can
	   separate them. */
	[data-indicators="bars"] [data-column-number] {
		--gitbutler-diff-number-start: calc(
			var(--gitbutler-diff-gutter-width) + var(--gitbutler-diff-gutter-seam)
		);

		background-image:
			linear-gradient(
				to right,
				transparent var(--gitbutler-diff-gutter-width),
				var(--gitbutler-diff-gutter-seam-color) var(--gitbutler-diff-gutter-width),
				var(--gitbutler-diff-gutter-seam-color) var(--gitbutler-diff-number-start),
				transparent var(--gitbutler-diff-number-start)
			),
			linear-gradient(
				to right,
				transparent var(--gitbutler-diff-number-start),
				var(--gitbutler-diff-number-film) var(--gitbutler-diff-number-start)
			);
	}

	slot[${GUTTER_SLOT_ATTRIBUTE}] {
		position: absolute;
		inset-block: 0;
		width: var(--gitbutler-diff-gutter-control-width);
		display: flex;
		align-items: center;
		justify-content: center;
	}

	/* The hunk's checkbox is the head of its band, so it stands over the column the band paints. */
	slot[${GUTTER_SLOT_KIND_ATTRIBUTE}="hunk"] {
		inset-inline-start: 0;
	}

	slot[${GUTTER_SLOT_KIND_ATTRIBUTE}="line"] {
		inset-inline-start: var(--gitbutler-diff-gutter-control-width);
	}

	/* The hunk column is one strip per hunk, drawn on every one of its lines: a ghost of itself
	   while the pointer is in its column, the pop fill once anything in the hunk is checked. Clicking
	   anywhere along it checks the hunk, and because the strip is exactly as tall as the hunk,
	   nothing has to explain how far that reaches. */
	[${HUNK_BAND_ATTRIBUTE}] {
		position: absolute;
		inset-block: 0;
		inset-inline-start: 0;
		width: var(--gitbutler-diff-gutter-control-width);
	}

	[${HUNK_BAND_ATTRIBUTE}][${GUTTER_HOVERED_ATTRIBUTE}] {
		background-color: color-mix(in srgb, currentColor 10%, transparent);
	}

	[${HUNK_BAND_ATTRIBUTE}][${HUNK_BAND_CHECKED_ATTRIBUTE}] {
		background-color: color-mix(in srgb, var(--fill-pop-bg) 60%, transparent);
	}

	/* One card per view, moved to whichever number cell is hovered, carrying the acts that belong
	   to a single line: the grip that drags it, and whatever the annotate slot brings. A translate
	   of its own width parks its start edge on the cell's end, so the numbers stay readable. Either
	   act alone is enough to be worth a card; with neither there is nothing to show. */
	[${ACTIONS_ATTRIBUTE}] {
		position: absolute;
		inset-block: 0;
		inset-inline-end: 0;
		translate: 80% 0;
		z-index: 4;
		display: none;
		align-items: center;
		height: 24px;
		margin-block: auto;
		padding: var(--gitbutler-diff-actions-padding);
		border-radius: var(--radius-card);
		background-color: var(--bg-1);
		box-shadow: var(--shadow-tooltip);
	}

	/* Written before the filled rule so that a line which cannot be dragged but can be annotated
	   still gets its card. */
	[${ACTIONS_ATTRIBUTE}][${ACTIONS_DRAGGABLE_ATTRIBUTE}] {
		display: if(style(--gitbutler-diff-gutter-can-drag: true): flex; else: none);
	}

	[${ACTIONS_ATTRIBUTE}][${ACTIONS_FILLED_ATTRIBUTE}] {
		display: flex;
	}

	/* The grip states itself quietly — the card is already an answer to hovering the line — and
	   firms up only once the pointer is on the grip itself. It appears only on lines a drag would
	   actually take: added and removed ones, never the context between them. */
	[${GUTTER_DRAG_HANDLE_ATTRIBUTE}] {
		/* The width below is the icon's own, so the rule beside it is added rather than taken. */
		box-sizing: content-box;
		display: none;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: 16px;
		height: 20px;
		opacity: 0.3;
		color: var(--text-1);
		cursor: grab;
	}

	[${ACTIONS_DRAGGABLE_ATTRIBUTE}] > [${GUTTER_DRAG_HANDLE_ATTRIBUTE}] {
		display: if(style(--gitbutler-diff-gutter-can-drag: true): flex; else: none);
	}

	[${GUTTER_DRAG_HANDLE_ATTRIBUTE}]:hover {
		opacity: 0.6;
	}

	[${GUTTER_DRAG_HANDLE_ATTRIBUTE}] > svg {
		width: 16px;
		height: 16px;
	}

	/* The grip and whatever the annotate slot brings are separate acts, so a rule stands between
	   them. It belongs to the grip, and so is gone on any line the grip itself is gone from. */
	[${ACTIONS_ATTRIBUTE}][${ACTIONS_FILLED_ATTRIBUTE}] > [${GUTTER_DRAG_HANDLE_ATTRIBUTE}] {
		margin-inline-end: 3px;
		padding-inline-end: 3px;
		border-inline-end: 1px solid var(--border-2);
	}

	slot[${COMMENT_SLOT_ATTRIBUTE}] {
		display: flex;
	}

	/* A checkbox states itself only where checking is what a press would do. The rest of the number
	   cell starts a line selection instead, so hovering it leaves the columns as they were: the hunk
	   checkbox answers its own column anywhere down the hunk, the line checkbox only its own cell. */
	slot[${GUTTER_SLOT_KIND_ATTRIBUTE}="hunk"][${GUTTER_HOVERED_ATTRIBUTE}],
	slot[${GUTTER_SLOT_KIND_ATTRIBUTE}="line"]:hover {
		--gitbutler-diff-gutter-checkbox-opacity: 1;
	}

	[data-indicators="bars"] [data-column-number]::before {
		inset-inline-start: calc(
			var(--gitbutler-diff-gutter-width) + var(--gitbutler-diff-gutter-seam)
		);
	}

	/* A mark is drawn as a box laid over the line rather than an outline of it, so that a run of
	   marked lines can close as one: each line drops the edge it shares with a marked neighbour,
	   leaving the dashes only around the outside. The lines of a run are siblings and equally tall,
	   so their side dashes keep step. */
	[${DRAG_PREVIEW_ATTRIBUTE}]::after,
	[${OPERATION_SOURCE_ATTRIBUTE}]::after {
		content: "";
		position: absolute;
		inset: 0;
		border-radius: var(--radius-md);
		pointer-events: none;
	}

	/* What the grip under the pointer is holding: the same dashed box the drag itself paints, in a
	   quieter weight. Declared first so a line that is already an operation source keeps the
	   stronger mark. */
	[${DRAG_PREVIEW_ATTRIBUTE}]::after {
		border: 1px dashed var(--text-1);
	}

	[${OPERATION_SOURCE_ATTRIBUTE}]::after {
		border: 2px dashed var(--border-1);
	}

	/* Only the ends of a run are corners; the edge a line shares with a marked neighbour goes, and
	   the rounding with it. */
	[${DRAG_PREVIEW_ATTRIBUTE}] + [${DRAG_PREVIEW_ATTRIBUTE}]::after,
	[${OPERATION_SOURCE_ATTRIBUTE}] + [${OPERATION_SOURCE_ATTRIBUTE}]::after {
		border-block-start: none;
		border-start-start-radius: 0;
		border-start-end-radius: 0;
	}

	[${DRAG_PREVIEW_ATTRIBUTE}]:has(+ [${DRAG_PREVIEW_ATTRIBUTE}])::after,
	[${OPERATION_SOURCE_ATTRIBUTE}]:has(+ [${OPERATION_SOURCE_ATTRIBUTE}])::after {
		border-block-end: none;
		border-end-start-radius: 0;
		border-end-end-radius: 0;
	}
`;

type OnPostRender<T> = NonNullable<CodeViewOptions<T>["onPostRender"]>;

type InternalGutterStore<T> = GutterStore & {
	onPostRender: OnPostRender<T>;
	syncOperationSources: () => void;
	cleanUp: () => void;
};

type GetHunkAddress = (target: DiffLineTarget) => HunkAddress | null;

const removeGutterControls = (host: HTMLElement): void => {
	for (const control of host.shadowRoot?.querySelectorAll(
		`[${GUTTER_SLOT_ATTRIBUTE}], [${HUNK_BAND_ATTRIBUTE}], [${ACTIONS_ATTRIBUTE}]`,
	) ?? [])
		control.remove();
};

const clearLineMarks = (host: HTMLElement): void => {
	for (const line of host.shadowRoot?.querySelectorAll<HTMLElement>(
		`[${OPERATION_SOURCE_ATTRIBUTE}], [${DRAG_PREVIEW_ATTRIBUTE}]`,
	) ?? []) {
		line.removeAttribute(OPERATION_SOURCE_ATTRIBUTE);
		line.removeAttribute(DRAG_PREVIEW_ATTRIBUTE);
	}
};

const sourcesContainLine = (sources: Array<Address> | null, line: HunkAddress): boolean =>
	sources?.some((source) => source._tag === "Hunk" && hunkAddressContainsLine(source, line)) ??
	false;

const sourcesKey = (sources: Array<Address> | null): string =>
	sources?.map(addressIdentityKey).join("|") ?? "";

const dragPreviewSourcesByHost = new Map<HTMLElement, Array<Address>>();
const dragPreviewKeysByHost = new Map<HTMLElement, string>();
const dragPreviewSyncByHost = new Map<HTMLElement, () => void>();

/**
 * Marks the lines the grip under the pointer would take, before the drag starts.
 *
 * Only the drag module knows that rule — checked lines, else the selection the line falls in, else
 * the whole containing hunk — and only the gutter walk knows which element each address landed on,
 * so the two meet here. Passing null clears the marks.
 */
export const setDiffDragPreviewSources = (
	host: HTMLElement,
	sources: Array<Address> | null,
): void => {
	const key = sourcesKey(sources);
	if ((dragPreviewKeysByHost.get(host) ?? "") === key) return;

	if (sources === null || sources.length === 0) {
		dragPreviewSourcesByHost.delete(host);
		dragPreviewKeysByHost.delete(host);
	} else {
		dragPreviewSourcesByHost.set(host, sources);
		dragPreviewKeysByHost.set(host, key);
	}
	dragPreviewSyncByHost.get(host)?.();
};

const forgetDragPreview = (host: HTMLElement): void => {
	dragPreviewSourcesByHost.delete(host);
	dragPreviewKeysByHost.delete(host);
	dragPreviewSyncByHost.delete(host);
};

const keepPointerDownOutOfLineSelection = (event: PointerEvent): void => {
	// Pierre treats every descendant of a number cell as a line-selection target. The grip still
	// needs its native dragstart to bubble to the registered host, but its initiating press does not,
	// and pressing the hunk band is a check rather than a selection.
	event.stopPropagation();
};

type ActionsCard = {
	card: HTMLElement;
	slot: HTMLSlotElement;
};

const createActionsCard = (slotName: string): ActionsCard => {
	const card = document.createElement("span");
	card.setAttribute(ACTIONS_ATTRIBUTE, "");

	const dragHandle = document.createElement("span");
	dragHandle.setAttribute(GUTTER_DRAG_HANDLE_ATTRIBUTE, "");
	dragHandle.setAttribute("aria-hidden", "true");
	// The grip is what the native drag starts on; the host below it is what Atlaskit registered.
	dragHandle.setAttribute("draggable", "true");
	dragHandle.addEventListener("pointerdown", keepPointerDownOutOfLineSelection);
	// Bundled app asset, same source the Icon component draws from.
	dragHandle.innerHTML = assert(icons.get("drag-vertical"));

	const slot = document.createElement("slot");
	slot.name = slotName;
	slot.setAttribute(COMMENT_SLOT_ATTRIBUTE, "");
	// Comments are optional; the card shows for them alone when dragging is unavailable.
	slot.addEventListener("slotchange", () => {
		card.toggleAttribute(ACTIONS_FILLED_ATTRIBUTE, slot.assignedNodes().length > 0);
	});

	card.append(dragHandle, slot);
	return { card, slot };
};

const ensureHunkBand = (
	cell: HTMLElement,
	groupKey: string,
	onClick: (event: MouseEvent) => void,
): HTMLElement => {
	let band = cell.querySelector<HTMLElement>(`:scope > [${HUNK_BAND_ATTRIBUTE}]`);
	if (!band) {
		band = document.createElement("span");
		band.setAttribute(HUNK_BAND_ATTRIBUTE, "");
		band.setAttribute("aria-hidden", "true");
		cell.prepend(band);
	}
	// Set on every pass, not just on creation: a band that outlives a hot reload is reused, and a
	// band that only looks clickable is worse than none. The listener is a stable reference, so
	// re-adding it is a no-op.
	band.setAttribute(GUTTER_GROUP_ATTRIBUTE, groupKey);
	band.addEventListener("pointerdown", keepPointerDownOutOfLineSelection);
	band.addEventListener("click", onClick);
	return band;
};

const createGutterStore = <T>(
	getOnPostRender: () => OnPostRender<T>,
	getLineAddress: () => GetHunkAddress,
	getParentAddress: () => GetHunkAddress,
	getOperationSourceAddresses: () => Array<Address> | null,
	getOnCheckHunk: () => (
		address: HunkAddress,
		lineAddresses: Array<Extract<Address, { _tag: "Hunk" }>>,
		shiftKey: boolean,
	) => void,
	getOnCheckLine: () => (address: HunkAddress, shiftKey: boolean) => void,
	isAddressChecked: (address: Extract<Address, { _tag: "Hunk" }>) => boolean,
): InternalGutterStore<T> => {
	const listeners = new Set<() => void>();
	const targets = new Map<HTMLElement, GutterTarget>();
	const hoveredGroupKeys = new Map<HTMLElement, string>();
	const controlsByGroupByHost = new Map<HTMLElement, Map<string, Array<HTMLElement>>>();
	const removeHoverListenersByHost = new Map<HTMLElement, () => void>();
	const itemIdsByHost = new Map<HTMLElement, string>();
	const actionCardsByHost = new Map<HTMLElement, ActionsCard>();
	const bandsByGroupByHost = new Map<HTMLElement, Map<string, Array<HTMLElement>>>();
	const checkedGroupsByHost = new Map<HTMLElement, Set<string>>();
	const checkableGroupsByHost = new Map<HTMLElement, Set<string>>();
	const commentTargetsByHost = new Map<HTMLElement, DiffLineTarget>();
	let nextKey = 0;
	let notificationQueued = false;
	let snapshot: ReadonlyArray<GutterTarget> = [];

	const publish = (): void => {
		snapshot = Array.from(targets.values());
		if (notificationQueued) return;

		notificationQueued = true;
		queueMicrotask(() => {
			notificationQueued = false;
			for (const listener of listeners) listener();
		});
	};

	/** The hunk whose own column the pointer is in, which is the only hunk the gutter answers for. */
	const setHoveredGroup = (host: HTMLElement, groupKey: string | undefined): void => {
		const previousGroupKey = hoveredGroupKeys.get(host);
		if (previousGroupKey === groupKey) return;

		const controlsByGroup = controlsByGroupByHost.get(host);
		for (const control of controlsByGroup?.get(previousGroupKey ?? "") ?? [])
			control.removeAttribute(GUTTER_HOVERED_ATTRIBUTE);
		for (const control of controlsByGroup?.get(groupKey ?? "") ?? [])
			control.setAttribute(GUTTER_HOVERED_ATTRIBUTE, "");

		if (groupKey === undefined) hoveredGroupKeys.delete(host);
		else hoveredGroupKeys.set(host, groupKey);
	};

	const paintBand = (band: HTMLElement, checked: boolean): void => {
		band.toggleAttribute(HUNK_BAND_CHECKED_ATTRIBUTE, checked);
	};

	// The column is one tall checkbox: its band answers a click anywhere down the hunk with the same
	// act as the checkbox at the top of it. A drag never produces a click, so the two gestures do not
	// have to be told apart here.
	const handleBandClick = (event: MouseEvent): void => {
		const band = event.currentTarget;
		if (!(band instanceof HTMLElement)) return;

		const groupKey = band.getAttribute(GUTTER_GROUP_ATTRIBUTE);
		const root = band.getRootNode();
		const host = root instanceof ShadowRoot ? root.host : null;
		if (groupKey === null || !(host instanceof HTMLElement)) return;
		if (!checkableGroupsByHost.get(host)?.has(groupKey)) return;

		const group = targets.get(host)?.groups.find((candidate) => candidate.key === groupKey);
		if (!group) return;

		// This lands inside a line-number cell, but it is not a line selection.
		event.stopPropagation();
		event.preventDefault();
		getOnCheckHunk()(
			group.parentAddress,
			group.lines.map((line) => line.address),
			event.shiftKey,
		);
	};

	const setGroupChecked = (host: HTMLElement, groupKey: string, checked: boolean): void => {
		const checkedGroups = checkedGroupsByHost.get(host) ?? new Set<string>();
		if (checkedGroups.has(groupKey) === checked) return;

		if (checked) checkedGroups.add(groupKey);
		else checkedGroups.delete(groupKey);
		checkedGroupsByHost.set(host, checkedGroups);
		for (const band of bandsByGroupByHost.get(host)?.get(groupKey) ?? []) paintBand(band, checked);
	};

	const setGroupCheckable = (host: HTMLElement, groupKey: string, checkable: boolean): void => {
		const groups = checkableGroupsByHost.get(host) ?? new Set<string>();
		if (groups.has(groupKey) === checkable) return;

		if (checkable) groups.add(groupKey);
		else groups.delete(groupKey);
		checkableGroupsByHost.set(host, groups);
	};

	/**
	 * The hunk whose column the pointer is in: its band, or the checkbox standing in the band's
	 * place on the hunk's first line. Anywhere else in the number cell answers a press with a line
	 * selection, so the gutter has nothing to offer there.
	 */
	const hunkColumnGroupKeyFromEvent = (event: Event): string | undefined => {
		const column = event
			.composedPath()
			.find(
				(target): target is HTMLElement =>
					target instanceof HTMLElement &&
					(target.hasAttribute(HUNK_BAND_ATTRIBUTE) ||
						target.getAttribute(GUTTER_SLOT_KIND_ATTRIBUTE) === "hunk"),
			);
		return column?.getAttribute(GUTTER_GROUP_ATTRIBUTE) ?? undefined;
	};

	/**
	 * A press on a line checkbox that has since moved onto other lines, painting each one it
	 * crosses to whatever the press did to the line it started on.
	 */
	let checkDrag: {
		host: HTMLElement;
		startAddress: Extract<Address, { _tag: "Hunk" }>;
		startKey: string;
		checked: boolean;
		painted: Set<string>;
		moved: boolean;
	} | null = null;

	const lineAddressAtPoint = (
		host: HTMLElement,
		clientX: number,
		clientY: number,
	): Extract<Address, { _tag: "Hunk" }> | null => {
		const shadowRoot = host.shadowRoot;
		const itemId = itemIdsByHost.get(host);
		if (!shadowRoot || itemId === undefined) return null;

		const element = shadowRoot.elementFromPoint(clientX, clientY);
		if (!(element instanceof HTMLElement)) return null;

		// A checkbox is the host's own child assigned to a slot, so the cell holding it is only an
		// ancestor of the slot, never of the checkbox.
		const cell =
			element.closest<HTMLElement>("[data-column-number]") ??
			element.assignedSlot?.closest<HTMLElement>("[data-column-number]") ??
			null;
		if (!cell) return null;

		const target = diffLineTargetFromElement({ element: cell, itemId });
		if (target?.lineType !== "change") return null;

		const address = getLineAddress()(target);
		return address ? hunkAddress(address) : null;
	};

	const paintCheckDragLine = (address: Extract<Address, { _tag: "Hunk" }>): void => {
		if (!checkDrag) return;

		const key = addressIdentityKey(address);
		if (checkDrag.painted.has(key)) return;

		checkDrag.painted.add(key);
		// Each line is set, not toggled, so a line the press already agrees with is left alone.
		if (isAddressChecked(address) !== checkDrag.checked) getOnCheckLine()(address, false);
	};

	const handleCheckDragMove = (event: PointerEvent): void => {
		if (!checkDrag) return;

		const address = lineAddressAtPoint(checkDrag.host, event.clientX, event.clientY);
		if (!address) return;

		const key = addressIdentityKey(address);
		// Until the press reaches another line it is still a click, and the checkbox answers it.
		if (!checkDrag.moved && key === checkDrag.startKey) return;

		if (!checkDrag.moved) {
			checkDrag.moved = true;
			checkDrag.host.setAttribute(CHECK_DRAG_ATTRIBUTE, "");
			paintCheckDragLine(checkDrag.startAddress);
		}
		paintCheckDragLine(address);
	};

	/**
	 * Opens the paint gesture. It sits on the slot rather than the tree above it because Pierre
	 * reads a press on any descendant of a number cell as the start of a line-range selection, and
	 * its listener is on an ancestor of this one.
	 */
	const handleLineSlotPointerDown = (event: Event): void => {
		if (!(event instanceof PointerEvent) || event.button !== 0) return;

		const slot = event.currentTarget;
		if (!(slot instanceof HTMLElement)) return;

		const root = slot.getRootNode();
		const host = root instanceof ShadowRoot ? root.host : null;
		if (!(host instanceof HTMLElement)) return;

		const address = lineAddressAtPoint(host, event.clientX, event.clientY);
		if (!address) return;

		event.stopPropagation();
		endCheckDrag();
		checkDrag = {
			host,
			startAddress: address,
			startKey: addressIdentityKey(address),
			checked: !isAddressChecked(address),
			painted: new Set(),
			moved: false,
		};
		window.addEventListener("pointermove", handleCheckDragMove);
		window.addEventListener("pointerup", endCheckDrag);
		window.addEventListener("pointercancel", endCheckDrag);
	};

	const swallowCheckDragClick = (event: Event): void => {
		event.stopPropagation();
		event.preventDefault();
	};

	const endCheckDrag = (): void => {
		if (!checkDrag) return;

		const { host, moved } = checkDrag;
		checkDrag = null;
		host.removeAttribute(CHECK_DRAG_ATTRIBUTE);
		window.removeEventListener("pointermove", handleCheckDragMove);
		window.removeEventListener("pointerup", endCheckDrag);
		window.removeEventListener("pointercancel", endCheckDrag);
		if (!moved) return;

		// The press still lands as a click on the checkbox it started on, which would undo the first
		// line the drag painted.
		window.addEventListener("click", swallowCheckDragClick, { capture: true, once: true });
		setTimeout(() => window.removeEventListener("click", swallowCheckDragClick, { capture: true }));
	};

	const ensureHoverListeners = (host: HTMLElement, shadowRoot: ShadowRoot): void => {
		if (removeHoverListenersByHost.has(host)) return;

		/** The card stands for one line-number cell, so it goes wherever that cell is not. */
		const hideActions = () => {
			commentTargetsByHost.delete(host);
			actionCardsByHost.get(host)?.card.remove();
		};

		// CSS can see the hovered column, but cannot match its dynamic hunk key to the rest of the
		// hunk's own band or to the checkbox at the top of it. The line checkbox stays local to :hover.
		const handlePointerOver = (event: Event) => {
			setHoveredGroup(host, hunkColumnGroupKeyFromEvent(event));

			const cell = event
				.composedPath()
				.find(
					(target): target is HTMLElement =>
						target instanceof HTMLElement && target.hasAttribute("data-column-number"),
				);
			const itemId = itemIdsByHost.get(host);
			// The code beside the numbers is still inside the view, so leaving the view is not what
			// takes the card back. A pointer anywhere off the cells is already off the line it named.
			if (!cell || itemId === undefined) return hideActions();

			const target = diffLineTargetFromElement({ element: cell, itemId });
			const actions = actionCardsByHost.get(host);
			if (!target || !actions) return hideActions();

			commentTargetsByHost.set(host, target);
			// A context line has no hunk to hand over, so the card arrives there without its grip.
			actions.card.toggleAttribute(ACTIONS_DRAGGABLE_ATTRIBUTE, target.lineType === "change");
			// Pointer events keep arriving as the pointer crosses the card's own children.
			// Re-appending the card to the cell it already sits in would take it out of the
			// document and put it back, losing the hover its grip is styled by.
			if (actions.card.parentElement !== cell) cell.appendChild(actions.card);
		};
		const handlePointerLeave = () => {
			setHoveredGroup(host, undefined);
			hideActions();
		};
		shadowRoot.addEventListener("pointerover", handlePointerOver);
		host.addEventListener("pointerleave", handlePointerLeave);
		// The drag module tells us what the grip holds; repainting it is this walk's job.
		dragPreviewSyncByHost.set(host, () => {
			const itemId = itemIdsByHost.get(host);
			if (itemId !== undefined) syncTarget(host, itemId);
		});
		removeHoverListenersByHost.set(host, () => {
			if (checkDrag?.host === host) endCheckDrag();
			shadowRoot.removeEventListener("pointerover", handlePointerOver);
			host.removeEventListener("pointerleave", handlePointerLeave);
		});
	};

	const removeTarget = (host: HTMLElement): void => {
		removeHoverListenersByHost.get(host)?.();
		removeHoverListenersByHost.delete(host);
		hoveredGroupKeys.delete(host);
		controlsByGroupByHost.delete(host);
		itemIdsByHost.delete(host);
		actionCardsByHost.delete(host);
		bandsByGroupByHost.delete(host);
		checkedGroupsByHost.delete(host);
		checkableGroupsByHost.delete(host);
		commentTargetsByHost.delete(host);
		forgetDragPreview(host);
		removeGutterControls(host);
		clearLineMarks(host);
		if (!targets.delete(host)) return;
		publish();
	};

	const syncTarget = (host: HTMLElement, itemId: string): void => {
		const shadowRoot = host.shadowRoot;
		if (!shadowRoot) return removeTarget(host);

		const cells = shadowRoot.querySelectorAll<HTMLElement>("[data-column-number]");
		if (cells.length === 0) return removeTarget(host);

		const existing = targets.get(host);
		const key = existing?.key ?? nextKey++;
		let actions = actionCardsByHost.get(host);
		if (!actions) {
			actions = createActionsCard(`gitbutler-diff-comment-${key}`);
			actionCardsByHost.set(host, actions);
		}
		const comment =
			existing?.comment ??
			({
				slotName: actions.slot.name,
				getTarget: () => commentTargetsByHost.get(host),
			} satisfies GutterTarget["comment"]);
		const groupsByKey = new Map<string, GutterCheckboxGroup>();
		const controlsByGroup = new Map<string, Array<HTMLElement>>();
		const bandsByGroup = new Map<string, Array<HTMLElement>>();
		const checkedGroups = checkedGroupsByHost.get(host);
		const usedControls = new Set<HTMLElement>();
		const operationSources = getOperationSourceAddresses();
		const dragPreviewSources = dragPreviewSourcesByHost.get(host) ?? null;
		itemIdsByHost.set(host, itemId);
		ensureHoverListeners(host, shadowRoot);

		for (const [index, cell] of cells.entries()) {
			const target = diffLineTargetFromElement({ element: cell, itemId });
			if (target?.lineType !== "change") continue;

			const lineAddress = getLineAddress()(target);
			const parentAddress = getParentAddress()(target);
			if (!lineAddress || !parentAddress) continue;

			const checkedLineAddress = hunkAddress(lineAddress);
			const checkedParentAddress = hunkAddress(parentAddress);
			const lineIndex = cell.getAttribute("data-line-index");
			const lineType = cell.getAttribute("data-line-type");
			const codeLine =
				lineIndex !== null && lineType !== null
					? shadowRoot.querySelector<HTMLElement>(
							`[data-line][data-line-index="${CSS.escape(lineIndex)}"][data-line-type="${CSS.escape(lineType)}"]`,
						)
					: null;
			codeLine?.toggleAttribute(
				OPERATION_SOURCE_ATTRIBUTE,
				sourcesContainLine(operationSources, checkedLineAddress),
			);
			codeLine?.toggleAttribute(
				DRAG_PREVIEW_ATTRIBUTE,
				sourcesContainLine(dragPreviewSources, checkedLineAddress),
			);
			const groupKey = addressIdentityKey(checkedParentAddress);
			const lineSlotName = `gitbutler-diff-gutter-line-${key}-${index}`;
			const group = groupsByKey.get(groupKey);
			if (group) {
				group.lines.push({ address: checkedLineAddress, slotName: lineSlotName });
			} else {
				const parentSlotName = `gitbutler-diff-gutter-hunk-${key}-${index}`;
				groupsByKey.set(groupKey, {
					key: groupKey,
					parentAddress: checkedParentAddress,
					parentSlotName,
					lines: [{ address: checkedLineAddress, slotName: lineSlotName }],
				});

				let parentSlot = cell.querySelector<HTMLSlotElement>(
					`:scope > slot[${GUTTER_SLOT_KIND_ATTRIBUTE}="hunk"]`,
				);
				if (!parentSlot) {
					parentSlot = document.createElement("slot");
					parentSlot.setAttribute(GUTTER_SLOT_ATTRIBUTE, "");
					parentSlot.setAttribute(GUTTER_SLOT_KIND_ATTRIBUTE, "hunk");
					cell.prepend(parentSlot);
				}
				parentSlot.name = parentSlotName;
				parentSlot.setAttribute(GUTTER_GROUP_ATTRIBUTE, groupKey);
				parentSlot.toggleAttribute(
					GUTTER_HOVERED_ATTRIBUTE,
					hoveredGroupKeys.get(host) === groupKey,
				);
				const groupControls = controlsByGroup.get(groupKey);
				if (groupControls) groupControls.push(parentSlot);
				else controlsByGroup.set(groupKey, [parentSlot]);
				usedControls.add(parentSlot);
			}

			let slot = cell.querySelector<HTMLSlotElement>(
				`:scope > slot[${GUTTER_SLOT_KIND_ATTRIBUTE}="line"]`,
			);
			if (!slot) {
				slot = document.createElement("slot");
				slot.setAttribute(GUTTER_SLOT_ATTRIBUTE, "");
				slot.setAttribute(GUTTER_SLOT_KIND_ATTRIBUTE, "line");
				cell.prepend(slot);
			}
			slot.name = lineSlotName;
			slot.setAttribute(GUTTER_GROUP_ATTRIBUTE, groupKey);
			// A stable reference, so a slot that outlives a hot reload takes this only once.
			slot.addEventListener("pointerdown", handleLineSlotPointerDown);
			usedControls.add(slot);

			const band = ensureHunkBand(cell, groupKey, handleBandClick);
			paintBand(band, checkedGroups?.has(groupKey) ?? false);
			band.toggleAttribute(GUTTER_HOVERED_ATTRIBUTE, hoveredGroupKeys.get(host) === groupKey);
			const groupBands = bandsByGroup.get(groupKey);
			if (groupBands) groupBands.push(band);
			else bandsByGroup.set(groupKey, [band]);
			const groupControls = controlsByGroup.get(groupKey);
			if (groupControls) groupControls.push(band);
			else controlsByGroup.set(groupKey, [band]);
			usedControls.add(band);
		}
		controlsByGroupByHost.set(host, controlsByGroup);
		bandsByGroupByHost.set(host, bandsByGroup);
		const hoveredGroupKey = hoveredGroupKeys.get(host);
		if (hoveredGroupKey !== undefined && !controlsByGroup.has(hoveredGroupKey))
			setHoveredGroup(host, undefined);

		for (const control of shadowRoot.querySelectorAll<HTMLElement>(
			`slot[${GUTTER_SLOT_ATTRIBUTE}], [${HUNK_BAND_ATTRIBUTE}]`,
		))
			if (!usedControls.has(control)) control.remove();

		const groups = Array.from(groupsByKey.values());
		if (groups.length === 0) return removeTarget(host);

		if (
			existing?.groups.length === groups.length &&
			existing.groups.every((group, index) => {
				const next = groups[index];
				return (
					next !== undefined &&
					group.key === next.key &&
					group.parentSlotName === next.parentSlotName &&
					group.lines.length === next.lines.length &&
					group.lines.every((line, lineIndex) => {
						const nextLine = next.lines[lineIndex];
						return (
							nextLine !== undefined &&
							line.slotName === nextLine.slotName &&
							addressIdentityKey(line.address) === addressIdentityKey(nextLine.address)
						);
					})
				);
			})
		)
			return;

		targets.set(host, { host, key, groups, comment });
		publish();
	};

	return {
		getSnapshot: () => snapshot,
		setGroupChecked,
		setGroupCheckable,
		onPostRender: (host, instance, phase, context) => {
			if (phase === "unmount" || context.type !== "diff") removeTarget(host);
			else syncTarget(host, context.item.id);
			// CodeView exposes this callback as file/diff overloads; forward the exact invocation.
			Reflect.apply(getOnPostRender(), undefined, [host, instance, phase, context]);
		},
		syncOperationSources: () => {
			for (const [host, itemId] of itemIdsByHost) syncTarget(host, itemId);
		},
		subscribe: (listener) => {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
		cleanUp: () => {
			for (const host of targets.keys()) {
				removeHoverListenersByHost.get(host)?.();
				forgetDragPreview(host);
				removeGutterControls(host);
				clearLineMarks(host);
			}
			removeHoverListenersByHost.clear();
			hoveredGroupKeys.clear();
			controlsByGroupByHost.clear();
			itemIdsByHost.clear();
			actionCardsByHost.clear();
			bandsByGroupByHost.clear();
			checkedGroupsByHost.clear();
			checkableGroupsByHost.clear();
			commentTargetsByHost.clear();
			targets.clear();
			snapshot = [];
		},
	};
};

export const useDiffGutterCheckboxes = <T>(
	onPostRender: OnPostRender<T>,
	getLineAddress: GetHunkAddress,
	getParentAddress: GetHunkAddress,
	projectId: string,
	onCheckLine: (address: HunkAddress, shiftKey: boolean) => void,
	onCheckHunk: (
		address: HunkAddress,
		lineAddresses: Array<Extract<Address, { _tag: "Hunk" }>>,
		shiftKey: boolean,
	) => void,
	onComment?: (target: DiffLineTarget) => void,
): {
	onPostRender: OnPostRender<T>;
	portals: ReactElement;
} => {
	const onPostRenderRef = useRef(onPostRender);
	onPostRenderRef.current = onPostRender;
	const getLineAddressRef = useRef(getLineAddress);
	getLineAddressRef.current = getLineAddress;
	const getParentAddressRef = useRef(getParentAddress);
	getParentAddressRef.current = getParentAddress;
	const appStore = useAppStore();
	const projectIdRef = useRef(projectId);
	projectIdRef.current = projectId;
	const onCheckHunkRef = useRef(onCheckHunk);
	onCheckHunkRef.current = onCheckHunk;
	const onCheckLineRef = useRef(onCheckLine);
	onCheckLineRef.current = onCheckLine;

	const storeRef = useRef<InternalGutterStore<T>>(null);
	storeRef.current ??= createGutterStore(
		() => onPostRenderRef.current,
		() => getLineAddressRef.current,
		() => getParentAddressRef.current,
		() =>
			getOperationSources(
				projectSlice.selectors.selectPendingOperation(appStore.getState(), projectIdRef.current),
			),
		() => onCheckHunkRef.current,
		() => onCheckLineRef.current,
		(address) =>
			projectSlice.selectors.selectAddressChecked(
				appStore.getState(),
				projectIdRef.current,
				address,
			),
	);
	const store = storeRef.current;

	useLayoutEffect(() => {
		let previousMode = projectSlice.selectors.selectPendingOperation(
			appStore.getState(),
			projectId,
		);
		let previousSources = getOperationSources(previousMode);
		return appStore.subscribe(() => {
			const mode = projectSlice.selectors.selectPendingOperation(appStore.getState(), projectId);
			if (mode === previousMode) return;

			previousMode = mode;
			const sources = getOperationSources(mode);
			if (sources === previousSources) return;

			previousSources = sources;
			store.syncOperationSources();
		});
	}, [appStore, projectId, store]);
	useLayoutEffect(() => () => store.cleanUp(), [store]);

	return {
		onPostRender: store.onPostRender,
		portals: createElement(DiffGutterPortals, {
			projectId,
			store,
			onCheckLine,
			onCheckHunk,
			onComment,
		}),
	};
};
