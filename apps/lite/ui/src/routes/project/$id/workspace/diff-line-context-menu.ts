import type { CodeViewHandle } from "@pierre/diffs/react";
import { useEffectEvent, useLayoutEffect, type RefObject } from "react";
import {
	diffLineTargetFromElement,
	LINE_NUMBER_ATTRIBUTES,
	type DiffLineTarget,
} from "./diff-line-target.ts";

export type DiffLineContextMenuTarget = {
	event: MouseEvent;
} & DiffLineTarget;

const contextMenuTarget = <T>(
	event: MouseEvent,
	viewerRef: RefObject<CodeViewHandle<T> | null>,
): DiffLineContextMenuTarget | null => {
	// Pierre renders every diff item into its own open shadow root. Context-menu events are
	// composed, so inspecting their path lets us delegate from the stable CodeView container
	// instead of observing renders or attaching listeners to every line number.
	const path = event.composedPath();
	// A row is a gutter cell and the code beside it, each numbering the line in its own way. The
	// menu belongs to the whole row, so take whichever half was clicked.
	const lineElement = path.find(
		(target): target is HTMLElement =>
			target instanceof HTMLElement &&
			LINE_NUMBER_ATTRIBUTES.some((attribute) => target.hasAttribute(attribute)),
	);
	if (!lineElement) return null;

	const item = viewerRef.current
		?.getInstance()
		?.getRenderedItems()
		.find(({ element }) => path.includes(element));
	if (item?.type !== "diff") return null;

	const target = diffLineTargetFromElement({ element: lineElement, itemId: item.id });
	return target ? { event, ...target } : null;
};

export const useDiffLineContextMenu = <T>({
	viewerRef,
	onContextMenu,
}: {
	viewerRef: RefObject<CodeViewHandle<T> | null>;
	onContextMenu: (target: DiffLineContextMenuTarget) => void;
}): void => {
	const handleContextMenu = useEffectEvent((event: MouseEvent) => {
		const target = contextMenuTarget(event, viewerRef);
		if (target) onContextMenu(target);
	});

	useLayoutEffect(() => {
		const container = viewerRef.current?.getInstance()?.getContainerElement();
		if (!container) return;

		container.addEventListener("contextmenu", handleContextMenu);
		return () => container.removeEventListener("contextmenu", handleContextMenu);
	}, [viewerRef]);
};
