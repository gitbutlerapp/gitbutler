import { dropTargetForElements } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import { type RefCallback, useEffect, useEffectEvent, useRef, useState } from "react";

type LibParams = Parameters<typeof dropTargetForElements>[0];

export const useDroppable = ({
	canDrop: canDropProp,
	getData: getDataProp,
}: Pick<LibParams, "canDrop" | "getData">): [boolean, RefCallback<HTMLElement>] => {
	const ref = useRef<HTMLElement>(null);
	const [isDragOver, setIsDragOver] = useState(false);
	const getData: LibParams["getData"] = useEffectEvent((x) => getDataProp?.(x) ?? {});
	const canDrop: LibParams["canDrop"] = useEffectEvent((x) => canDropProp?.(x) ?? true);

	useEffect(() => {
		const element = ref.current;
		if (!element) return;

		return dropTargetForElements({
			element,
			canDrop,
			getData,
			onDragEnter: () => {
				setIsDragOver(true);
			},
			onDragLeave: () => {
				setIsDragOver(false);
			},
			onDrop: () => {
				setIsDragOver(false);
			},
		});
	}, []);

	return [
		isDragOver,
		(element) => {
			ref.current = element;
		},
	];
};
