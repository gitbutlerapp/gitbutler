import { draggable } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import { centerUnderPointer } from "@atlaskit/pragmatic-drag-and-drop/element/center-under-pointer";
import { setCustomNativeDragPreview } from "@atlaskit/pragmatic-drag-and-drop/element/set-custom-native-drag-preview";
import {
	type ReactNode,
	type RefCallback,
	useEffect,
	useEffectEvent,
	useRef,
	useState,
} from "react";
import { createRoot } from "react-dom/client";

type LibParams = Parameters<typeof draggable>[0];

export const useDraggable = ({
	getInitialData: getInitialDataProp,
	canDrag: canDragProp,
	preview,
}: Pick<LibParams, "canDrag" | "getInitialData"> & {
	preview: ReactNode;
}): [boolean, RefCallback<HTMLElement>] => {
	const ref = useRef<HTMLElement>(null);
	const [isDragging, setIsDragging] = useState(false);
	const getInitialData: LibParams["getInitialData"] = useEffectEvent(
		(args) => getInitialDataProp?.(args) ?? {},
	);
	const canDrag: LibParams["canDrag"] = useEffectEvent((args) => canDragProp?.(args) ?? true);
	const onGenerateDragPreview = useEffectEvent(
		({ nativeSetDragImage }: { nativeSetDragImage: DataTransfer["setDragImage"] | null }) => {
			setCustomNativeDragPreview({
				nativeSetDragImage,
				getOffset: centerUnderPointer,
				render: ({ container }) => {
					const root = createRoot(container);
					root.render(preview);
					return () => {
						root.unmount();
					};
				},
			});
		},
	);

	useEffect(() => {
		const element = ref.current;
		if (!element) return;

		return draggable({
			element,
			canDrag,
			getInitialData,
			onGenerateDragPreview,
			onDragStart: () => {
				setIsDragging(true);
			},
			onDrop: () => {
				setIsDragging(false);
			},
		});
	}, []);

	return [
		isDragging,
		(element) => {
			ref.current = element;
		},
	];
};
