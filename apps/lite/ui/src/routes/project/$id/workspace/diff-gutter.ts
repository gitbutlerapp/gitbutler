import type { CodeViewOptions } from "@pierre/diffs";
import { createElement, type ReactElement, useLayoutEffect, useRef } from "react";
import {
	hunkAddress,
	hunkAddressContainsLine,
	addressIdentityKey,
	type HunkAddress,
	type Address,
} from "#ui/addresses.ts";
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
const COMMENT_SLOT_ATTRIBUTE = "data-gitbutler-diff-comment-slot";
const OPERATION_SOURCE_ATTRIBUTE = "data-gitbutler-operation-source";

export const diffGutterUnsafeCSS = `
	:host {
		--gitbutler-diff-gutter-control-width: 1lh;
		--gitbutler-diff-gutter-width: calc(3 * var(--gitbutler-diff-gutter-control-width));
	}

	[data-column-number] {
		padding-left: calc(2ch + var(--gitbutler-diff-gutter-width));
	}

	slot[${GUTTER_SLOT_ATTRIBUTE}] {
		position: absolute;
		inset-block: 0;
		width: var(--gitbutler-diff-gutter-control-width);
		display: flex;
		align-items: center;
		justify-content: center;
	}

	slot[${GUTTER_SLOT_KIND_ATTRIBUTE}="hunk"] {
		inset-inline-start: var(--gitbutler-diff-gutter-control-width);
	}

	slot[${GUTTER_SLOT_KIND_ATTRIBUTE}="line"] {
		inset-inline-start: calc(2 * var(--gitbutler-diff-gutter-control-width));
	}

	[${GUTTER_DRAG_HANDLE_ATTRIBUTE}] {
		position: absolute;
		inset-block: 0;
		inset-inline-start: 0;
		width: var(--gitbutler-diff-gutter-control-width);
		display: if(style(--gitbutler-diff-gutter-can-drag: true): flex; else: none);
		align-items: center;
		justify-content: center;
		opacity: var(--gitbutler-diff-gutter-drag-opacity, 0);
	}

	[${GUTTER_DRAG_HANDLE_ATTRIBUTE}]::before {
		content: "";
		width: 8px;
		height: 12px;
		background: radial-gradient(circle, currentColor 1px, transparent 1.5px) 0 0 / 4px 4px;
	}

	slot[${COMMENT_SLOT_ATTRIBUTE}] {
		position: absolute;
		inset-block: 0;
		inset-inline-end: 0;
		display: flex;
		z-index: 4;
	}

	slot[${GUTTER_SLOT_KIND_ATTRIBUTE}="hunk"][${GUTTER_HOVERED_ATTRIBUTE}],
	[data-column-number]:hover > slot[${GUTTER_SLOT_KIND_ATTRIBUTE}="line"] {
		--gitbutler-diff-gutter-checkbox-opacity: 1;
	}

	[data-column-number]:hover > [${GUTTER_DRAG_HANDLE_ATTRIBUTE}] {
		--gitbutler-diff-gutter-drag-opacity: 1;
	}

	[data-indicators="bars"] [data-column-number]::before {
		inset-inline-start: var(--gitbutler-diff-gutter-width);
	}

	[${OPERATION_SOURCE_ATTRIBUTE}] {
		outline: 2px dashed var(--border-1);
		outline-offset: -2px;
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
		`[${GUTTER_SLOT_ATTRIBUTE}], [${GUTTER_DRAG_HANDLE_ATTRIBUTE}], [${COMMENT_SLOT_ATTRIBUTE}]`,
	) ?? [])
		control.remove();
};

const clearOperationSources = (host: HTMLElement): void => {
	for (const line of host.shadowRoot?.querySelectorAll<HTMLElement>(
		`[${OPERATION_SOURCE_ATTRIBUTE}]`,
	) ?? [])
		line.removeAttribute(OPERATION_SOURCE_ATTRIBUTE);
};

const isOperationSourceLine = (sources: Array<Address> | null, line: HunkAddress): boolean =>
	sources?.some((source) => source._tag === "Hunk" && hunkAddressContainsLine(source, line)) ??
	false;

const keepDragHandlePointerDownOutOfLineSelection = (event: PointerEvent): void => {
	// Pierre treats every descendant of a number cell as a line-selection target. The grip still
	// needs its native dragstart to bubble to the registered host, but its initiating press does not.
	event.stopPropagation();
};

const createGutterStore = <T>(
	getOnPostRender: () => OnPostRender<T>,
	getLineAddress: () => GetHunkAddress,
	getParentAddress: () => GetHunkAddress,
	getOperationSourceAddresses: () => Array<Address> | null,
): InternalGutterStore<T> => {
	const listeners = new Set<() => void>();
	const targets = new Map<HTMLElement, GutterTarget>();
	const hoveredGroupKeys = new Map<HTMLElement, string>();
	const controlsByGroupByHost = new Map<HTMLElement, Map<string, Array<HTMLElement>>>();
	const removeHoverListenersByHost = new Map<HTMLElement, () => void>();
	const itemIdsByHost = new Map<HTMLElement, string>();
	const commentSlotsByHost = new Map<HTMLElement, HTMLSlotElement>();
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

	const gutterGroupKeyFromEvent = (event: Event): string | undefined => {
		const cell = event
			.composedPath()
			.find(
				(target): target is HTMLElement =>
					target instanceof HTMLElement && target.hasAttribute("data-column-number"),
			);
		return (
			cell
				?.querySelector<HTMLSlotElement>(`:scope > slot[${GUTTER_SLOT_ATTRIBUTE}]`)
				?.getAttribute(GUTTER_GROUP_ATTRIBUTE) ?? undefined
		);
	};

	const ensureHoverListeners = (host: HTMLElement, shadowRoot: ShadowRoot): void => {
		if (removeHoverListenersByHost.has(host)) return;

		// CSS can see the hovered number cell, but cannot match its dynamic hunk key to the parent
		// checkbox at the top of the group. The line checkbox and drag handle stay local to :hover.
		const handlePointerOver = (event: Event) => {
			setHoveredGroup(host, gutterGroupKeyFromEvent(event));

			const cell = event
				.composedPath()
				.find(
					(target): target is HTMLElement =>
						target instanceof HTMLElement && target.hasAttribute("data-column-number"),
				);
			const itemId = itemIdsByHost.get(host);
			if (!cell || itemId === undefined) return;

			const target = diffLineTargetFromElement({ element: cell, itemId });
			const slot = commentSlotsByHost.get(host);
			if (!target || !slot) return;

			commentTargetsByHost.set(host, target);
			cell.appendChild(slot);
		};
		const handlePointerLeave = () => {
			setHoveredGroup(host, undefined);
			commentTargetsByHost.delete(host);
			commentSlotsByHost.get(host)?.remove();
		};
		shadowRoot.addEventListener("pointerover", handlePointerOver);
		host.addEventListener("pointerleave", handlePointerLeave);
		removeHoverListenersByHost.set(host, () => {
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
		commentSlotsByHost.delete(host);
		commentTargetsByHost.delete(host);
		removeGutterControls(host);
		clearOperationSources(host);
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
		let commentSlot = commentSlotsByHost.get(host);
		if (!commentSlot) {
			commentSlot = document.createElement("slot");
			commentSlot.name = `gitbutler-diff-comment-${key}`;
			commentSlot.setAttribute(COMMENT_SLOT_ATTRIBUTE, "");
			commentSlotsByHost.set(host, commentSlot);
		}
		const comment =
			existing?.comment ??
			({
				slotName: commentSlot.name,
				getTarget: () => commentTargetsByHost.get(host),
			} satisfies GutterTarget["comment"]);
		const groupsByKey = new Map<string, GutterCheckboxGroup>();
		const controlsByGroup = new Map<string, Array<HTMLElement>>();
		const usedControls = new Set<HTMLElement>();
		const operationSources = getOperationSourceAddresses();
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
				isOperationSourceLine(operationSources, checkedLineAddress),
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
			usedControls.add(slot);

			let dragHandle = cell.querySelector<HTMLElement>(
				`:scope > [${GUTTER_DRAG_HANDLE_ATTRIBUTE}]`,
			);
			if (!dragHandle) {
				dragHandle = document.createElement("span");
				dragHandle.setAttribute(GUTTER_DRAG_HANDLE_ATTRIBUTE, "");
				dragHandle.setAttribute("aria-hidden", "true");
				dragHandle.addEventListener("pointerdown", keepDragHandlePointerDownOutOfLineSelection);
				cell.prepend(dragHandle);
			}
			dragHandle.setAttribute(GUTTER_GROUP_ATTRIBUTE, groupKey);
			usedControls.add(dragHandle);
		}
		controlsByGroupByHost.set(host, controlsByGroup);
		const hoveredGroupKey = hoveredGroupKeys.get(host);
		if (hoveredGroupKey !== undefined && !controlsByGroup.has(hoveredGroupKey))
			setHoveredGroup(host, undefined);

		for (const control of shadowRoot.querySelectorAll<HTMLElement>(
			`slot[${GUTTER_SLOT_ATTRIBUTE}], [${GUTTER_DRAG_HANDLE_ATTRIBUTE}]`,
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
				removeGutterControls(host);
				clearOperationSources(host);
			}
			removeHoverListenersByHost.clear();
			hoveredGroupKeys.clear();
			controlsByGroupByHost.clear();
			itemIdsByHost.clear();
			commentSlotsByHost.clear();
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

	const storeRef = useRef<InternalGutterStore<T>>(null);
	storeRef.current ??= createGutterStore(
		() => onPostRenderRef.current,
		() => getLineAddressRef.current,
		() => getParentAddressRef.current,
		() =>
			getOperationSources(
				projectSlice.selectors.selectPendingOperation(appStore.getState(), projectIdRef.current),
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
