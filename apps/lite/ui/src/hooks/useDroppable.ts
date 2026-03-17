import { dropTargetForElements } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import { type RefCallback, useEffect, useEffectEvent, useRef, useState } from "react";

type LibParams = Parameters<typeof dropTargetForElements>[0];

export const useDroppable = ({
	canDrop: canDropProp,
	getData: getDataProp,
	disabled = false,
}: Pick<LibParams, "canDrop" | "getData"> & {
	disabled?: boolean;
}): [boolean, RefCallback<HTMLElement>] => {
	const ref = useRef<HTMLElement>(null);
	const [isDropTarget, setIsDropTarget] = useState(false);
	const getData: LibParams["getData"] = useEffectEvent((x) => getDataProp?.(x) ?? {});
	const canDrop: LibParams["canDrop"] = useEffectEvent((x) => canDropProp?.(x) ?? true);

	useEffect(() => {
		const element = ref.current;
		if (!element || disabled) return;

		return dropTargetForElements({
			element,
			canDrop,
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

	return [
		isDropTarget,
		(element) => {
			ref.current = element;
		},
	];
};
