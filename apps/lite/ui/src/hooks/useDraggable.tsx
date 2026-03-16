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

export const useDraggable = <TData extends Record<string, unknown>>({
	data,
	preview,
	disabled = false,
}: {
	data: TData;
	preview: ReactNode;
	disabled?: boolean;
}): {
	ref: RefCallback<HTMLElement>;
	isDragging: boolean;
} => {
	const ref = useRef<HTMLElement>(null);
	const [isDragging, setIsDragging] = useState(false);
	const getInitialData = useEffectEvent(() => data);
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
		if (!element || disabled) return;

		return draggable({
			element,
			getInitialData,
			onGenerateDragPreview,
			onDragStart: () => {
				setIsDragging(true);
			},
			onDrop: () => {
				setIsDragging(false);
			},
		});
	}, [disabled]);

	return {
		ref: (element) => {
			ref.current = element;
		},
		isDragging,
	};
};
