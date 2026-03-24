import { dropTargetForElements } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import { type RefCallback, useEffect, useEffectEvent, useRef, useState } from "react";

type LibParams = Parameters<typeof dropTargetForElements>[0];
type GetDataParams = Parameters<NonNullable<LibParams["getData"]>>;

export const useDroppable = <TData extends Record<string | symbol, unknown>>(
	getDataProp: (...args: GetDataParams) => TData | null,
): [TData | null, RefCallback<HTMLElement>] => {
	const ref = useRef<HTMLElement>(null);
	const [data, setData] = useState<TData | null>(null);
	const getData = useEffectEvent((...args: GetDataParams) => getDataProp(...args));
	const canDrop: LibParams["canDrop"] = useEffectEvent((args) => getData(args) !== null);

	useEffect(() => {
		const element = ref.current;
		if (!element) return;

		return dropTargetForElements({
			element,
			canDrop,
			getData: (args) => getData(args) ?? {},
			onDragEnter: ({ self }) => {
				setData(self.data as TData);
			},
			onDrag: ({ self }) => {
				setData(self.data as TData);
			},
			onDragLeave: () => {
				setData(null);
			},
			onDrop: () => {
				setData(null);
			},
		});
	}, []);

	return [
		data,
		(element) => {
			ref.current = element;
		},
	];
};
