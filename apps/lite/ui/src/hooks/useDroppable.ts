import { dropTargetForElements } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import { type RefCallback, useEffect, useEffectEvent, useRef, useState } from "react";

export const useDroppable = <TData extends Record<string | symbol, unknown>>({
	canDrop,
	getData: getDataProp,
	disabled = false,
}: {
	canDrop: (dragData: unknown) => boolean;
	getData: (dragData: unknown) => TData;
	disabled?: boolean;
}): [boolean, RefCallback<HTMLElement>] => {
	const ref = useRef<HTMLElement>(null);
	const [isDropTarget, setIsDropTarget] = useState(false);
	const getData = useEffectEvent((dragData: unknown) => getDataProp(dragData));
	const canDropForSource = useEffectEvent((dragData: unknown) => canDrop(dragData));

	useEffect(() => {
		const element = ref.current;
		if (!element || disabled) return;

		return dropTargetForElements({
			element,
			canDrop: ({ source }) => canDropForSource(source.data),
			getData: ({ source }) => getData(source.data),
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

	return [
		isDropTarget,
		(element) => {
			ref.current = element;
		},
	];
};
