import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { cancelMode } from "#ui/use-cursor.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import {
	hunkOperand,
	hunkOperandContainsLine,
	type FileParent,
	type HunkOperand,
	type Operand,
} from "#ui/operands.ts";
import { pointerTransferMode } from "#ui/outline/mode.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppStore } from "#ui/store.ts";
import {
	draggable,
	type ElementGetFeedbackArgs,
} from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import { centerUnderPointer } from "@atlaskit/pragmatic-drag-and-drop/element/center-under-pointer";
import { setCustomNativeDragPreview } from "@atlaskit/pragmatic-drag-and-drop/element/set-custom-native-drag-preview";
import type { CodeViewOptions } from "@pierre/diffs";
import { useQueryClient } from "@tanstack/react-query";
import { createElement, useLayoutEffect, useRef } from "react";
import { createRoot } from "react-dom/client";
import type { DragData } from "./DragData.ts";
import { parseDragData } from "./DragData.ts";
import { DragPreview } from "./OperationSourceC.tsx";
import { operandsLabel } from "./operandLabel.ts";
import { diffLineTargetFromElement, type DiffLineTarget } from "./diff-line-target.ts";

const HUNK_DRAG_HANDLE_ATTRIBUTE = "data-hunk-drag-handle";

type OnPostRender<T> = NonNullable<CodeViewOptions<T>["onPostRender"]>;

type Registration = {
	itemId: string;
	cleanup: () => void;
};

const hunkLineAtPoint = (
	host: HTMLElement,
	itemId: string,
	input: ElementGetFeedbackArgs["input"],
): DiffLineTarget | null => {
	const element = host.shadowRoot?.elementFromPoint(input.clientX, input.clientY);
	const lineNumberElement = element
		?.closest<HTMLElement>(`[${HUNK_DRAG_HANDLE_ATTRIBUTE}]`)
		?.closest("[data-column-number]");
	if (!(lineNumberElement instanceof HTMLElement)) return null;

	return diffLineTargetFromElement({ element: lineNumberElement, itemId });
};

const syncHunkDragHandles = (host: HTMLElement): void => {
	const shadowRoot = host.shadowRoot;
	if (!shadowRoot) return;

	for (const element of shadowRoot.querySelectorAll<HTMLElement>(`[${HUNK_DRAG_HANDLE_ATTRIBUTE}]`))
		element.setAttribute("draggable", "true");
};

const cleanHunkDragHandles = (host: HTMLElement): void => {
	for (const element of host.shadowRoot?.querySelectorAll<HTMLElement>(
		`[${HUNK_DRAG_HANDLE_ATTRIBUTE}]`,
	) ?? [])
		element.removeAttribute("draggable");
};

export const useDiffHunkDrag = <T>({
	projectId,
	fileParent,
	getHunkOperand,
	getLineOperand,
	getSelectedOperands,
}: {
	projectId: string;
	fileParent: FileParent;
	getHunkOperand: (target: DiffLineTarget) => HunkOperand | null;
	getLineOperand: (target: DiffLineTarget) => HunkOperand | null;
	getSelectedOperands: () => Array<Extract<Operand, { _tag: "Hunk" }>>;
}): OnPostRender<T> => {
	const store = useAppStore();
	const queryClient = useQueryClient();

	const config = {
		projectId,
		dispatch: store.dispatch,
		canDrag: () => {
			if (fileParent._tag === "Branch") return false;

			const mode = projectSlice.selectors.selectOutlineModeState(store.getState(), projectId);
			return mode._tag !== "InlineEdit";
		},
		getHeadInfoIndex: () => {
			const headInfo = queryClient.getQueryData(headInfoQueryOptions(projectId).queryKey);
			return headInfo ? getHeadInfoIndex(headInfo) : null;
		},
		getHunkOperand,
		getLineOperand,
		getSelectedOperands,
	};
	const configRef = useRef(config);
	configRef.current = config;
	const registrationsRef = useRef<Map<HTMLElement, Registration>>(new Map());

	const onPostRenderRef = useRef<OnPostRender<T>>(null);
	onPostRenderRef.current ??= (host, _instance, phase, context): void => {
		const registrations = registrationsRef.current;
		const existing = registrations.get(host);

		if (phase === "unmount") {
			existing?.cleanup();
			registrations.delete(host);
			cleanHunkDragHandles(host);
			return;
		}

		syncHunkDragHandles(host);
		if (existing) {
			existing.itemId = context.item.id;
			return;
		}

		const registration: Registration = {
			itemId: context.item.id,
			cleanup: () => {},
		};
		const resolveSources = (input: ElementGetFeedbackArgs["input"]): DragData["sources"] | null => {
			const target = hunkLineAtPoint(host, registration.itemId, input);
			if (!target) return null;

			const lineOperand = configRef.current.getLineOperand(target);
			const hunk = configRef.current.getHunkOperand(target);
			if (!lineOperand || !hunk) return null;

			const line = hunkOperand(lineOperand);
			const selected = configRef.current.getSelectedOperands();
			const state = store.getState();
			if (projectSlice.selectors.selectOperandChecked(state, projectId, line))
				return projectSlice.selectors.selectCheckedOperands(state, projectId);

			return selected.some((source) => hunkOperandContainsLine(source, line))
				? selected
				: [hunkOperand(hunk)];
		};

		registration.cleanup = draggable({
			element: host,
			canDrag: ({ input }) => configRef.current.canDrag() && resolveSources(input) !== null,
			getInitialData: ({ input }): DragData => ({
				sources: resolveSources(input) ?? [],
			}),
			onGenerateDragPreview: ({ nativeSetDragImage, source }) => {
				const sources = parseDragData(source.data)?.sources;
				if (!sources) return;

				setCustomNativeDragPreview({
					nativeSetDragImage,
					getOffset: centerUnderPointer,
					render: ({ container }) => {
						const headInfoIndex = configRef.current.getHeadInfoIndex();
						if (!headInfoIndex) return;

						const root = createRoot(container);
						root.render(
							createElement(DragPreview, null, operandsLabel({ operands: sources, headInfoIndex })),
						);
						return () => root.unmount();
					},
				});
			},
			onDragStart: ({ source }) => {
				const config = configRef.current;
				const sources = parseDragData(source.data)?.sources;
				if (!sources) return;

				config.dispatch(
					projectSlice.actions.enterTransferMode({
						projectId: config.projectId,
						mode: pointerTransferMode({
							sources,
							target: null,
							placement: null,
						}),
					}),
				);
			},
			onDrop: ({ location }) => {
				if (location.current.dropTargets.length > 0) return;

				cancelMode();
			},
		});

		// Native drag originates on the marked shadow children. Atlaskit still needs the host
		// registered because the composed dragstart event is retargeted to it at document.
		host.removeAttribute("draggable");
		registrations.set(host, registration);
	};

	useLayoutEffect(() => {
		const registrations = registrationsRef.current;
		return () => {
			for (const [host, registration] of registrations) {
				registration.cleanup();
				cleanHunkDragHandles(host);
			}
			registrations.clear();
		};
	}, []);

	return onPostRenderRef.current;
};
