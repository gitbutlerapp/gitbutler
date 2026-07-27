import type { CodeViewHandle } from "@pierre/diffs/react";
import { useEffectEvent, useLayoutEffect, type RefObject } from "react";
import { diffLineTargetFromElement, type DiffLineTarget } from "./diff-line-target.ts";

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
	const lineNumberElement = path.find(
		(target): target is HTMLElement =>
			target instanceof HTMLElement && target.hasAttribute("data-column-number"),
	);
	if (!lineNumberElement) return null;

	const item = viewerRef.current
		?.getInstance()
		?.getRenderedItems()
		.find(({ element }) => path.includes(element));
	if (item?.type !== "diff") return null;

	const target = diffLineTargetFromElement({ element: lineNumberElement, itemId: item.id });
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
