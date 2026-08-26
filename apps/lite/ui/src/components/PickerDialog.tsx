/**
 * @file Based on https://base-ui.com/react/components/autocomplete#command-palette
 */

import { Autocomplete, Dialog } from "@base-ui/react";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
	type CSSProperties,
	type ReactNode,
	type RefObject,
	useCallback,
	useDeferredValue,
	useImperativeHandle,
	useMemo,
	useRef,
	useState,
} from "react";
import { classes } from "#ui/components/classes.ts";
import uiStyles from "#ui/components/ui.module.css";
import styles from "./PickerDialog.module.css";

export type PickerDialogGroup<T> = {
	value: string;
	items: Array<T>;
};

type VirtualRow<T> =
	| { _tag: "Group"; group: PickerDialogGroup<T> }
	| { _tag: "Item"; group: PickerDialogGroup<T>; item: T; itemIndex: number };

type VirtualizerHandle = {
	itemCount: number;
	highlightEdgeItem: (edge: "start" | "end") => void;
	highlightPageItem: (currentItemIndex: number | null, direction: -1 | 1) => void;
	scrollToItemIndex: (
		index: number,
		options: Parameters<ReturnType<typeof useVirtualizer>["scrollToIndex"]>[1],
	) => void;
};

type VirtualizedListAreaProps<T> = {
	emptyLabel: string;
	getItemKey: (item: T) => string;
	getItemLabel: (item: T) => string;
	getItemType: (item: T, group: PickerDialogGroup<T>) => ReactNode;
	onSelectItem: (item: T) => void;
	statusLabel?: string;
	virtualizerRef: RefObject<VirtualizerHandle | null>;
};

const VirtualizedListArea = <T,>({
	emptyLabel,
	getItemKey,
	getItemLabel,
	getItemType,
	onSelectItem,
	statusLabel,
	virtualizerRef,
}: VirtualizedListAreaProps<T>) => {
	const scrollElementRef = useRef<HTMLDivElement | null>(null);
	const filteredGroups = Autocomplete.useFilteredItems<PickerDialogGroup<T>>();

	// React Compiler leaves components using useVirtualizer uncompiled because its returned
	// functions cannot be memoised safely:
	//   https://github.com/TanStack/virtual/issues/1119
	const { virtualRows, virtualRowIndexByItemIndex } = useMemo(() => {
		let itemIndex = 0;
		const virtualRows: Array<VirtualRow<T>> = [];
		const virtualRowIndexByItemIndex = new Map<number, number>();

		for (const group of filteredGroups) {
			virtualRows.push({ _tag: "Group", group });
			for (const item of group.items) {
				virtualRowIndexByItemIndex.set(itemIndex, virtualRows.length);
				virtualRows.push({ _tag: "Item", group, item, itemIndex: itemIndex++ });
			}
		}

		return { virtualRows, virtualRowIndexByItemIndex };
	}, [filteredGroups]);

	const getVirtualRowKey = useCallback(
		(index: number) => {
			const row = virtualRows[index];
			if (row === undefined) return index;
			return row._tag === "Group"
				? `group:${row.group.value}`
				: `item:${row.group.value}:${getItemKey(row.item)}`;
		},
		[getItemKey, virtualRows],
	);

	const itemCount = virtualRows.length - filteredGroups.length;

	const virtualizer = useVirtualizer({
		directDomUpdates: true,
		directDomUpdatesMode: "transform",
		count: virtualRows.length,
		getScrollElement: () => scrollElementRef.current,
		estimateSize: () => 32,
		getItemKey: getVirtualRowKey,
		overscan: 4,
		paddingStart: 8,
		paddingEnd: 8,
		scrollPaddingStart: 8,
		scrollPaddingEnd: 8,
	});
	const highlightRequestRef = useRef(0);

	const highlightItemIndex = (index: number) => {
		const virtualRowIndex = virtualRowIndexByItemIndex.get(index);
		if (virtualRowIndex === undefined) return;

		const request = ++highlightRequestRef.current;
		virtualizer.scrollToIndex(virtualRowIndex, { align: "auto" });
		let remainingFrames = 2;

		const highlightMountedItem = () => {
			if (request !== highlightRequestRef.current) return;
			const item = scrollElementRef.current?.querySelector<HTMLElement>(
				`[data-item-index="${index}"]`,
			);
			if (item) {
				// Base UI has no public highlighted-index setter, so use its item handler once
				// the virtualizer has mounted the target option.
				item.dispatchEvent(new MouseEvent("mousemove", { bubbles: true }));
				return;
			}
			if (remainingFrames-- > 0) requestAnimationFrame(highlightMountedItem);
		};

		queueMicrotask(highlightMountedItem);
	};

	useImperativeHandle(virtualizerRef, () => ({
		itemCount,
		highlightEdgeItem: (edge) => {
			if (itemCount > 0) highlightItemIndex(edge === "start" ? 0 : itemCount - 1);
		},
		highlightPageItem: (currentItemIndex, direction) => {
			if (itemCount === 0) return;

			const scrollElement = scrollElementRef.current;
			const visibleItemCount = scrollElement
				? virtualizer
						.getVirtualItems()
						.filter(
							(row) =>
								virtualRows[row.index]?._tag === "Item" &&
								row.end > scrollElement.scrollTop &&
								row.start < scrollElement.scrollTop + scrollElement.clientHeight,
						).length
				: 1;
			const currentIndex = currentItemIndex ?? (direction === 1 ? 0 : itemCount - 1);
			const pageItemCount = Math.max(1, visibleItemCount - 1);
			const targetIndex = Math.max(
				0,
				Math.min(itemCount - 1, currentIndex + direction * pageItemCount),
			);
			highlightItemIndex(targetIndex);
		},
		scrollToItemIndex: (index, options) => {
			const virtualRowIndex = virtualRowIndexByItemIndex.get(index);
			if (virtualRowIndex !== undefined) virtualizer.scrollToIndex(virtualRowIndex, options);
		},
	}));

	return (
		<div ref={scrollElementRef} className={classes(uiStyles.scroller, styles.listArea)}>
			<div className={styles.listContent}>
				<Autocomplete.Status>
					{statusLabel !== undefined ? <div className={styles.empty}>{statusLabel}</div> : null}
				</Autocomplete.Status>
				<Autocomplete.Empty>
					{statusLabel === undefined ? <div className={styles.empty}>{emptyLabel}</div> : null}
				</Autocomplete.Empty>

				<Autocomplete.List className={styles.list}>
					{virtualRows.length > 0 && (
						<div
							role="presentation"
							ref={virtualizer.containerRef}
							className={styles.virtualContainer}
						>
							{virtualizer.getVirtualItems().map((virtualItem) => {
								const row = virtualRows[virtualItem.index];
								if (row === undefined) return null;

								const style: CSSProperties = {
									position: "absolute",
									top: 0,
									left: "0.5rem",
									width: "calc(100% - 1rem)",
									height: virtualItem.size,
								};

								if (row._tag === "Group") {
									return (
										<div
											key={virtualItem.key}
											ref={virtualizer.measureElement}
											data-index={virtualItem.index}
											role="presentation"
											className={styles.groupLabel}
											style={style}
										>
											{row.group.value}
										</div>
									);
								}

								const itemType = getItemType(row.item, row.group);
								return (
									<Autocomplete.Item
										key={virtualItem.key}
										index={row.itemIndex}
										data-index={virtualItem.index}
										data-item-index={row.itemIndex}
										ref={virtualizer.measureElement}
										className={styles.item}
										value={row.item}
										onClick={() => onSelectItem(row.item)}
										aria-setsize={itemCount}
										aria-posinset={row.itemIndex + 1}
										style={style}
									>
										<span className={styles.itemLabel}>{getItemLabel(row.item)}</span>
										{itemType !== undefined && <span className={styles.itemType}>{itemType}</span>}
									</Autocomplete.Item>
								);
							})}
						</div>
					)}
				</Autocomplete.List>
			</div>
		</div>
	);
};

type Props<T> = {
	ariaLabel: string;
	closeLabel: string;
	emptyLabel: string;
	footerAction?: ReactNode;
	getItemKey: (item: T) => string;
	getItemLabel: (item: T) => string;
	getItemType: (item: T, group: PickerDialogGroup<T>) => ReactNode;
	itemToStringValue?: (item: T) => string;
	items: Array<PickerDialogGroup<T>>;
	onOpenChange: (open: boolean) => void;
	onSelectItem: (item: T) => void;
	open: boolean;
	placeholder: string;
	statusLabel?: string;
};

export const PickerDialog = <T,>({
	ariaLabel,
	closeLabel,
	emptyLabel,
	footerAction,
	getItemKey,
	getItemLabel,
	getItemType,
	itemToStringValue,
	items,
	onOpenChange,
	onSelectItem,
	open,
	placeholder,
	statusLabel,
}: Props<T>) => {
	const inputRef = useRef<HTMLInputElement | null>(null);
	const virtualizerRef = useRef<VirtualizerHandle | null>(null);
	const highlightedItemIndexRef = useRef<number | null>(null);
	const [inputValue, setInputValue] = useState("");
	const deferredInputValue = useDeferredValue(inputValue);

	return (
		<Dialog.Root open={open} onOpenChange={onOpenChange}>
			<Dialog.Portal>
				<Dialog.Backdrop className={styles.backdrop} />
				<Dialog.Viewport className={styles.viewport}>
					<Dialog.Popup className={styles.popup} aria-label={ariaLabel} initialFocus={inputRef}>
						<Autocomplete.Root
							items={items}
							inline
							open
							value={deferredInputValue}
							onValueChange={setInputValue}
							virtualized
							onItemHighlighted={(_, { reason, index }) => {
								highlightedItemIndexRef.current = index < 0 ? null : index;
								const virtualizer = virtualizerRef.current;
								if (!virtualizer || index < 0) return;

								const isStart = index === 0;
								const isEnd = index === virtualizer.itemCount - 1;
								const shouldScroll =
									reason === "none" || (reason === "keyboard" && (isStart || isEnd));
								if (shouldScroll) {
									queueMicrotask(() => {
										virtualizerRef.current?.scrollToItemIndex(index, {
											align: isEnd ? "start" : "end",
										});
									});
								}
							}}
							autoHighlight="always"
							keepHighlight
							itemToStringValue={itemToStringValue ?? getItemLabel}
						>
							<Autocomplete.Input
								ref={inputRef}
								value={inputValue}
								onKeyDown={(event) => {
									const virtualizer = virtualizerRef.current;
									if (!virtualizer) return;

									if (event.metaKey && event.key === "ArrowUp") {
										event.preventDefault();
										event.preventBaseUIHandler();
										virtualizer.highlightEdgeItem("start");
									} else if (event.metaKey && event.key === "ArrowDown") {
										event.preventDefault();
										event.preventBaseUIHandler();
										virtualizer.highlightEdgeItem("end");
									} else if (event.key === "PageUp") {
										event.preventDefault();
										event.preventBaseUIHandler();
										virtualizer.highlightPageItem(highlightedItemIndexRef.current, -1);
									} else if (event.key === "PageDown") {
										event.preventDefault();
										event.preventBaseUIHandler();
										virtualizer.highlightPageItem(highlightedItemIndexRef.current, 1);
									}
								}}
								className={styles.input}
								placeholder={placeholder}
								aria-label={placeholder}
							/>
							<Dialog.Close className={styles.visuallyHiddenClose}>{closeLabel}</Dialog.Close>

							<VirtualizedListArea
								emptyLabel={emptyLabel}
								getItemKey={getItemKey}
								getItemLabel={getItemLabel}
								getItemType={getItemType}
								onSelectItem={onSelectItem}
								statusLabel={statusLabel}
								virtualizerRef={virtualizerRef}
							/>

							<div className={styles.footer}>
								<div className={styles.footerLeft}>
									<span>Activate</span>
									<kbd className={styles.kbd}>Enter</kbd>
								</div>
								{footerAction}
							</div>
						</Autocomplete.Root>
					</Dialog.Popup>
				</Dialog.Viewport>
			</Dialog.Portal>
		</Dialog.Root>
	);
};
