import { dropTargetForElements } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import { type RefCallback, useEffect, useEffectEvent, useRef, useState } from "react";

export const useDroppable = <TData extends Record<string | symbol, unknown>>({
	canDrop,
	data,
	disabled = false,
}: {
	canDrop: (dragData: unknown) => boolean;
	data: TData;
	disabled?: boolean;
}): {
	ref: RefCallback<HTMLElement>;
	isDropTarget: boolean;
} => {
	const ref = useRef<HTMLElement>(null);
	const [isDropTarget, setIsDropTarget] = useState(false);
	const getData = useEffectEvent(() => data);
	const canDropForSource = useEffectEvent((dragData: unknown) => canDrop(dragData));

	useEffect(() => {
		const element = ref.current;
		if (!element || disabled) return;

		return dropTargetForElements({
			element,
			canDrop: ({ source }) => canDropForSource(source.data),
			getData,
			onDragEnter: () => {
				setIsDropTarget(true);
			},
			onDragLeave: () => {
				setIsDropTarget(false);
			},
			onDrop: () => {
				setIsDropTarget(false);
			},
		});
	}, [disabled]);

	return {
		ref: (element) => {
			ref.current = element;
		},
		isDropTarget,
	};
};
