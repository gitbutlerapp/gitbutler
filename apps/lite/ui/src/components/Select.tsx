import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { FieldLabelStyles, FieldRootStyles } from "#ui/components/Field.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import type { IconName } from "#ui/components/iconNames.ts";
import { Popup, PopupItem, PopupSearch } from "#ui/components/Popup.tsx";
import styles from "./Select.module.css";
import { Combobox, Field, Select as BaseSelect } from "@base-ui/react";
import type { CSSProperties, ReactNode } from "react";

/** @public */
export type SelectItem<Value extends string> = {
	value: Value;
	label: string;
	/** Leads the row in the list and the trigger while chosen — what kind of thing the choice is. */
	icon?: IconName;
	/**
	 * Leads the row and the trigger where a glyph from the icon set is not the right mark — a
	 * program's own image, say. Sits after `icon` when both are given.
	 */
	leading?: ReactNode;
	disabled?: boolean;
};

/** @public */
export type SelectProps<Value extends string> = {
	items: ReadonlyArray<SelectItem<Value>>;
	/**
	 * The field's label, above the control. Omit for a select the surface labels some other way,
	 * and give it an `aria-label` instead.
	 */
	label?: ReactNode;
	/**
	 * What the control reads while nothing is chosen. It is also the first row of the list, so the
	 * choice can be cleared from the same place it was made — except in a searchable list, which
	 * has no clearing row.
	 */
	placeholder?: string;
	/** Omit to leave the select uncontrolled. */
	value?: Value | null;
	defaultValue?: Value | null;
	onValueChange?: (value: Value | null) => void;
	disabled?: boolean;
	/** Identifies the field when a form is submitted. */
	name?: string;
	/**
	 * Leads the list with the search row every other long list in Lite leads with — the branch
	 * and commit target pickers — and filters the rows as the query is typed. For a list too long
	 * to scan, such as the syntax themes. The list opens below the trigger rather than over it,
	 * since the query row, not the chosen row, is what takes the trigger's place.
	 *
	 * @default false
	 */
	searchable?: boolean;
	/** What the search row reads while empty. @default "Search..." */
	searchPlaceholder?: string;
	/** The line the list shows when the query matches nothing. @default "Nothing found" */
	nothingFound?: string;
	className?: string;
	style?: CSSProperties;
	"aria-label"?: string;
};

/**
 * One choice from a short, fixed list: a terminal, a theme, a default branch. The trigger is a
 * field-sized outline button reading the current choice, led by that choice's icon or image if
 * it has one, and the list it opens is the same {@link Popup} every dropdown wears, with a tick
 * on the row that is chosen now.
 *
 * The list opens over the trigger so the chosen row sits where the trigger's text was, the way
 * a native select does; Base UI drops that overlap when the window has no room for it. Each row's
 * text is wrapped in Base UI's ItemText part, which is what the overlap measures against — a row
 * without it leaves the list to open plainly below the trigger.
 *
 * A list too long to scan takes `searchable`, which puts the {@link PopupSearch} row at its top
 * and filters the rows as the query is typed. Underneath that is a combobox rather than a select,
 * dressed the same way. For a list whose rows are not `{ value, label }` pairs — a branch, a
 * commit target — reach for the combobox directly instead.
 *
 * @public
 */
export const Select = <Value extends string>(props: SelectProps<Value>) => (
	<Field.Root render={<FieldRootStyles />} className={props.className} style={props.style}>
		{props.label !== undefined && (
			<Field.Label render={<FieldLabelStyles />}>{props.label}</Field.Label>
		)}
		{props.searchable ? <SearchableSelect {...props} /> : <PlainSelect {...props} />}
	</Field.Root>
);

/**
 * What the trigger holds: the chosen item's mark, its label (or the placeholder), and the chevron.
 * The mark sits outside the label on purpose: the plain select's overlap measures the label's box
 * against the chosen row's text, so anything inside it before the text shifts the whole list that
 * far to the left.
 */
const TriggerContent = <Value extends string>({
	item,
	children,
}: {
	item: SelectItem<Value> | undefined;
	/** The label part, which is the select's or the combobox's own. */
	children: ReactNode;
}) => (
	<>
		{item?.icon !== undefined && <Icon name={item.icon} />}
		{item?.leading}
		{children}
		<Icon name="chevron-down" />
	</>
);

const triggerClassName = classes(getButtonClassName({ variant: "outline" }), styles.trigger);

const PlainSelect = <Value extends string>({
	items,
	placeholder,
	value,
	defaultValue,
	onValueChange,
	disabled = false,
	name,
	"aria-label": ariaLabel,
}: SelectProps<Value>) => (
	<BaseSelect.Root<Value>
		items={items}
		value={value}
		defaultValue={defaultValue}
		onValueChange={onValueChange}
		disabled={disabled}
		name={name}
	>
		<BaseSelect.Trigger
			aria-label={ariaLabel}
			className={triggerClassName}
			render={(props, state) => {
				// Base UI hands over the value alone; the item is what carries the mark.
				const item = items.find((candidate) => candidate.value === state.value);
				return (
					<button type="button" {...props}>
						<TriggerContent item={item}>
							<BaseSelect.Value className={styles.value} placeholder={placeholder}>
								{item?.label ?? placeholder}
							</BaseSelect.Value>
						</TriggerContent>
					</button>
				);
			}}
		/>
		<BaseSelect.Portal>
			<BaseSelect.Positioner align="start" sideOffset={4}>
				<Popup anchored className={styles.popup} render={<BaseSelect.Popup />}>
					<BaseSelect.List className={styles.list}>
						{placeholder !== undefined && (
							<PopupItem
								className={styles.placeholderItem}
								render={<BaseSelect.Item value={null} />}
							>
								<BaseSelect.ItemText>{placeholder}</BaseSelect.ItemText>
							</PopupItem>
						)}
						{items.map((item) => (
							<PopupItem
								key={item.value}
								icon={item.icon}
								leading={item.leading}
								trailing="tick"
								className={styles.item}
								render={<BaseSelect.Item value={item.value} disabled={item.disabled} />}
							>
								<BaseSelect.ItemText>{item.label}</BaseSelect.ItemText>
							</PopupItem>
						))}
					</BaseSelect.List>
				</Popup>
			</BaseSelect.Positioner>
		</BaseSelect.Portal>
	</BaseSelect.Root>
);

/**
 * The same control over a combobox, so the list can lead with a search row. The combobox holds
 * whole items rather than values — that is what it filters on, by label — so the value crosses
 * the boundary as the item it names, and comes back as its value.
 */
const SearchableSelect = <Value extends string>({
	items,
	placeholder,
	value,
	defaultValue,
	onValueChange,
	disabled = false,
	name,
	searchPlaceholder = "Search...",
	nothingFound = "Nothing found",
	"aria-label": ariaLabel,
}: SelectProps<Value>) => {
	const itemFor = (candidate: Value | null | undefined) =>
		candidate === undefined ? undefined : (items.find((item) => item.value === candidate) ?? null);

	return (
		<Combobox.Root<SelectItem<Value>>
			items={items}
			value={itemFor(value)}
			defaultValue={itemFor(defaultValue)}
			onValueChange={(item) => onValueChange?.(item?.value ?? null)}
			isItemEqualToValue={(a, b) => a.value === b.value}
			autoHighlight
			disabled={disabled}
			name={name}
		>
			<Combobox.Trigger aria-label={ariaLabel} className={triggerClassName}>
				{/* The Value part renders no element of its own; it hands over the chosen item, which is
				    what carries the mark. */}
				<Combobox.Value>
					{(item: SelectItem<Value> | null) => (
						<TriggerContent item={item ?? undefined}>
							<span className={styles.value} data-placeholder={item === null || undefined}>
								{item?.label ?? placeholder}
							</span>
						</TriggerContent>
					)}
				</Combobox.Value>
			</Combobox.Trigger>
			<Combobox.Portal>
				<Combobox.Positioner align="start" sideOffset={4}>
					<Popup
						anchored
						className={classes(styles.popup, styles.searchablePopup)}
						render={<Combobox.Popup />}
					>
						<PopupSearch
							aria-label={searchPlaceholder}
							placeholder={searchPlaceholder}
							render={<Combobox.Input />}
						/>
						<Combobox.Empty>
							{/* A line, not the block: this dropdown is as narrow as its trigger, and the
							    illustration would fill it. */}
							<div className={classes("text-13", styles.empty)}>{nothingFound}</div>
						</Combobox.Empty>
						<Combobox.List className={styles.list}>
							{(item: SelectItem<Value>) => (
								<PopupItem
									key={item.value}
									icon={item.icon}
									leading={item.leading}
									trailing="tick"
									className={styles.item}
									render={<Combobox.Item value={item} disabled={item.disabled} />}
								>
									{item.label}
								</PopupItem>
							)}
						</Combobox.List>
					</Popup>
				</Combobox.Positioner>
			</Combobox.Portal>
		</Combobox.Root>
	);
};
