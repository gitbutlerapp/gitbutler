/**
 * @file Based on https://base-ui.com/react/components/autocomplete#command-palette
 */

import { Autocomplete } from "@base-ui/react/autocomplete";
import { Dialog } from "@base-ui/react/dialog";
import { ScrollArea } from "@base-ui/react/scroll-area";
import { ReactNode, useRef } from "react";
import styles from "./PickerDialog.module.css";

/** @public */
export type PickerDialogGroup<Item> = {
	value: string;
	items: Array<Item>;
};

export const PickerDialog = <Item,>({
	ariaLabel,
	closeLabel,
	emptyLabel,
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
}: {
	ariaLabel: string;
	closeLabel: string;
	emptyLabel: string;
	getItemKey: (item: Item) => string;
	getItemLabel: (item: Item) => string;
	getItemType: (item: Item, group: PickerDialogGroup<Item>) => ReactNode;
	itemToStringValue?: (item: Item) => string;
	items: Array<PickerDialogGroup<Item>>;
	onOpenChange: (open: boolean) => void;
	onSelectItem: (item: Item) => void;
	open: boolean;
	placeholder: string;
	statusLabel?: string;
}) => {
	const inputRef = useRef<HTMLInputElement | null>(null);

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
							autoHighlight="always"
							keepHighlight
							itemToStringValue={itemToStringValue ?? getItemLabel}
						>
							<Autocomplete.Input
								ref={inputRef}
								className={styles.input}
								placeholder={placeholder}
								aria-label={placeholder}
							/>
							<Dialog.Close className={styles.visuallyHiddenClose}>{closeLabel}</Dialog.Close>

							<ScrollArea.Root className={styles.listArea}>
								<ScrollArea.Viewport className={styles.listViewport}>
									<ScrollArea.Content className={styles.listContent}>
										<Autocomplete.Status>
											{statusLabel !== undefined ? (
												<div className={styles.empty}>{statusLabel}</div>
											) : null}
										</Autocomplete.Status>
										<Autocomplete.Empty>
											{statusLabel === undefined ? (
												<div className={styles.empty}>{emptyLabel}</div>
											) : null}
										</Autocomplete.Empty>

										<Autocomplete.List className={styles.list}>
											{(group: PickerDialogGroup<Item>) => (
												<Autocomplete.Group
													key={group.value}
													items={group.items}
													className={styles.group}
												>
													<Autocomplete.GroupLabel className={styles.groupLabel}>
														{group.value}
													</Autocomplete.GroupLabel>
													<Autocomplete.Collection>
														{(item: Item) => {
															const itemType = getItemType(item, group);
															return (
																<Autocomplete.Item
																	key={getItemKey(item)}
																	className={styles.item}
																	value={item}
																	onClick={() => onSelectItem(item)}
																>
																	<span className={styles.itemLabel}>{getItemLabel(item)}</span>
																	{itemType !== undefined && (
																		<span className={styles.itemType}>{itemType}</span>
																	)}
																</Autocomplete.Item>
															);
														}}
													</Autocomplete.Collection>
												</Autocomplete.Group>
											)}
										</Autocomplete.List>
									</ScrollArea.Content>
								</ScrollArea.Viewport>
								<ScrollArea.Scrollbar className={styles.scrollbar}>
									<ScrollArea.Thumb className={styles.scrollbarThumb} />
								</ScrollArea.Scrollbar>
							</ScrollArea.Root>

							<div className={styles.footer}>
								<div className={styles.footerLeft}>
									<span>Activate</span>
									<kbd className={styles.kbd}>Enter</kbd>
								</div>
							</div>
						</Autocomplete.Root>
					</Dialog.Popup>
				</Dialog.Viewport>
			</Dialog.Portal>
		</Dialog.Root>
	);
};
