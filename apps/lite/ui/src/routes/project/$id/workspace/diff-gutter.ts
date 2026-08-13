import type { CodeViewOptions } from "@pierre/diffs";
import { createElement, type ReactElement, useLayoutEffect, useRef } from "react";
import { hunkOperand, operandIdentityKey, type HunkOperand } from "#ui/operands.ts";
import {
	DiffGutterPortals,
	type GutterCheckboxGroup,
	type GutterStore,
	type GutterTarget,
} from "./DiffGutterPortals.tsx";
import { diffLineTargetFromElement, type DiffLineTarget } from "./diff-line-target.ts";

const GUTTER_SLOT_ATTRIBUTE = "data-gitbutler-diff-gutter-slot";
const GUTTER_GROUP_ATTRIBUTE = "data-gitbutler-diff-gutter-group";
const GUTTER_HOVERED_ATTRIBUTE = "data-gitbutler-diff-gutter-hovered";

export const diffGutterUnsafeCSS = `
	:host {
		--gitbutler-diff-gutter-width: 1lh;
	}

	[data-column-number] {
		padding-left: calc(2ch + var(--gitbutler-diff-gutter-width));
	}

	slot[${GUTTER_SLOT_ATTRIBUTE}] {
		position: absolute;
		inset-block: 0;
		inset-inline-start: 0;
		width: var(--gitbutler-diff-gutter-width);
		display: flex;
		align-items: center;
		justify-content: center;
	}

	slot[${GUTTER_HOVERED_ATTRIBUTE}] {
		--gitbutler-diff-gutter-checkbox-opacity: 1;
	}

	[data-indicators="bars"] [data-column-number]::before {
		inset-inline-start: var(--gitbutler-diff-gutter-width);
	}
`;

type OnPostRender<T> = NonNullable<CodeViewOptions<T>["onPostRender"]>;

type InternalGutterStore<T> = GutterStore & {
	onPostRender: OnPostRender<T>;
	cleanUp: () => void;
};

type GetHunkOperand = (target: DiffLineTarget) => HunkOperand | null;

const removeGutterSlots = (host: HTMLElement): void => {
	for (const slot of host.shadowRoot?.querySelectorAll(`[${GUTTER_SLOT_ATTRIBUTE}]`) ?? [])
		slot.remove();
};

const createGutterStore = <T>(
	getOnPostRender: () => OnPostRender<T>,
	getHunkOperand: () => GetHunkOperand,
): InternalGutterStore<T> => {
	const listeners = new Set<() => void>();
	const targets = new Map<HTMLElement, GutterTarget>();
	const hoveredGroupKeys = new Map<HTMLElement, string>();
	const slotsByGroupByHost = new Map<HTMLElement, Map<string, Array<HTMLSlotElement>>>();
	const removeHoverListenersByHost = new Map<HTMLElement, () => void>();
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

		const slotsByGroup = slotsByGroupByHost.get(host);
		for (const slot of slotsByGroup?.get(previousGroupKey ?? "") ?? [])
			slot.removeAttribute(GUTTER_HOVERED_ATTRIBUTE);
		for (const slot of slotsByGroup?.get(groupKey ?? "") ?? [])
			slot.setAttribute(GUTTER_HOVERED_ATTRIBUTE, "");

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

		// CSS can see the hovered number cell, but cannot match its dynamic hunk key to sibling slots.
		// Delegate once per diff host, then let CSS reveal every slot in the resolved hunk group.
		const handlePointerOver = (event: Event) =>
			setHoveredGroup(host, gutterGroupKeyFromEvent(event));
		const handlePointerLeave = () => setHoveredGroup(host, undefined);
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
		slotsByGroupByHost.delete(host);
		removeGutterSlots(host);
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
		const groupsByKey = new Map<string, GutterCheckboxGroup>();
		const slotsByGroup = new Map<string, Array<HTMLSlotElement>>();
		const usedSlots = new Set<HTMLSlotElement>();
		ensureHoverListeners(host, shadowRoot);

		for (const [index, cell] of cells.entries()) {
			const target = diffLineTargetFromElement({ element: cell, itemId });
			if (target?.lineType !== "change") continue;

			const operand = getHunkOperand()(target);
			if (!operand || operand.parent.parent._tag === "Branch") continue;

			const checkedOperand = hunkOperand(operand);
			const operandKey = operandIdentityKey(checkedOperand);
			const slotName = `gitbutler-diff-gutter-${key}-${index}`;
			const group = groupsByKey.get(operandKey);
			if (group) {
				group.slotNames.push(slotName);
			} else {
				groupsByKey.set(operandKey, {
					key: operandKey,
					operand: checkedOperand,
					slotNames: [slotName],
				});
			}

			const firstChild = cell.firstElementChild;
			let slot =
				firstChild instanceof HTMLSlotElement && firstChild.hasAttribute(GUTTER_SLOT_ATTRIBUTE)
					? firstChild
					: null;
			if (!slot) {
				slot = document.createElement("slot");
				slot.setAttribute(GUTTER_SLOT_ATTRIBUTE, "");
				cell.prepend(slot);
			}
			slot.name = slotName;
			slot.setAttribute(GUTTER_GROUP_ATTRIBUTE, operandKey);
			slot.toggleAttribute(GUTTER_HOVERED_ATTRIBUTE, hoveredGroupKeys.get(host) === operandKey);
			const groupSlots = slotsByGroup.get(operandKey);
			if (groupSlots) groupSlots.push(slot);
			else slotsByGroup.set(operandKey, [slot]);
			usedSlots.add(slot);
		}
		slotsByGroupByHost.set(host, slotsByGroup);
		const hoveredGroupKey = hoveredGroupKeys.get(host);
		if (hoveredGroupKey !== undefined && !slotsByGroup.has(hoveredGroupKey))
			setHoveredGroup(host, undefined);

		for (const slot of shadowRoot.querySelectorAll<HTMLSlotElement>(
			`slot[${GUTTER_SLOT_ATTRIBUTE}]`,
		))
			if (!usedSlots.has(slot)) slot.remove();

		const groups = Array.from(groupsByKey.values());
		if (groups.length === 0) return removeTarget(host);

		if (
			existing?.groups.length === groups.length &&
			existing.groups.every(
				(group, index) =>
					group.key === groups[index]?.key &&
					group.slotNames.length === groups[index].slotNames.length &&
					group.slotNames.every(
						(slotName, slotIndex) => slotName === groups[index]?.slotNames[slotIndex],
					),
			)
		)
			return;

		targets.set(host, { host, key, groups });
		publish();
	};

	return {
		getSnapshot: () => snapshot,
		onPostRender: (host, instance, phase, context) => {
			// CodeView exposes this callback as file/diff overloads; forward the exact invocation.
			Reflect.apply(getOnPostRender(), undefined, [host, instance, phase, context]);
			if (phase === "unmount" || context.type !== "diff") removeTarget(host);
			else syncTarget(host, context.item.id);
		},
		subscribe: (listener) => {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
		cleanUp: () => {
			for (const host of targets.keys()) {
				removeHoverListenersByHost.get(host)?.();
				removeGutterSlots(host);
			}
			removeHoverListenersByHost.clear();
			hoveredGroupKeys.clear();
			slotsByGroupByHost.clear();
			targets.clear();
			snapshot = [];
		},
	};
};

export const useDiffGutterCheckboxes = <T>(
	onPostRender: OnPostRender<T>,
	getHunkOperand: GetHunkOperand,
	projectId: string,
	onCheck: (event: { operand: HunkOperand; shiftKey: boolean }) => void,
): {
	onPostRender: OnPostRender<T>;
	portals: ReactElement;
} => {
	const onPostRenderRef = useRef(onPostRender);
	onPostRenderRef.current = onPostRender;
	const getHunkOperandRef = useRef(getHunkOperand);
	getHunkOperandRef.current = getHunkOperand;

	const storeRef = useRef<InternalGutterStore<T>>(null);
	storeRef.current ??= createGutterStore(
		() => onPostRenderRef.current,
		() => getHunkOperandRef.current,
	);
	const store = storeRef.current;

	useLayoutEffect(() => () => store.cleanUp(), [store]);

	return {
		onPostRender: store.onPostRender,
		portals: createElement(DiffGutterPortals, { projectId, store, onCheck }),
	};
};
