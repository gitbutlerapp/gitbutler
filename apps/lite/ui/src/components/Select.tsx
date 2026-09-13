import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { FieldLabelStyles, FieldRootStyles } from "#ui/components/Field.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import type { IconName } from "#ui/components/iconNames.ts";
import { Popup, PopupItem } from "#ui/components/Popup.tsx";
import styles from "./Select.module.css";
import { Field, Select as BaseSelect } from "@base-ui/react";
import type { CSSProperties, ReactNode } from "react";

/** @public */
export type SelectItem<Value extends string> = {
	value: Value;
	label: string;
	/** Leads the row in the list — what kind of thing the choice is. */
	icon?: IconName;
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
	 * choice can be cleared from the same place it was made.
	 */
	placeholder?: string;
	/** Omit to leave the select uncontrolled. */
	value?: Value | null;
	defaultValue?: Value | null;
	onValueChange?: (value: Value | null) => void;
	disabled?: boolean;
	/** Identifies the field when a form is submitted. */
	name?: string;
	className?: string;
	style?: CSSProperties;
	"aria-label"?: string;
};

/**
 * One choice from a short, fixed list: a terminal, a theme, a default branch. The trigger is a
 * field-sized outline button reading the current choice, and the list it opens is the same
 * {@link Popup} every dropdown wears, with a tick on the row that is chosen now.
 *
 * The list opens over the trigger so the chosen row sits where the trigger's text was, the way
 * a native select does; Base UI drops that overlap when the window has no room for it. Each row's
 * text is wrapped in Base UI's ItemText part, which is what the overlap measures against — a row
 * without it leaves the list to open plainly below the trigger.
 *
 * For a long or searchable list — a branch, a commit target — reach for a combobox and
 * {@link PopupSearch} instead: a select has no filter, and one is not worth adding to a control
 * whose whole list fits on screen.
 *
 * @public
 */
export const Select = <Value extends string>({
	items,
	label,
	placeholder,
	value,
	defaultValue,
	onValueChange,
	disabled = false,
	name,
	className,
	style,
	"aria-label": ariaLabel,
}: SelectProps<Value>) => (
	<Field.Root render={<FieldRootStyles />} className={className} style={style}>
		{label !== undefined && <Field.Label render={<FieldLabelStyles />}>{label}</Field.Label>}
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
				className={classes(getButtonClassName({ variant: "outline" }), styles.trigger)}
			>
				<BaseSelect.Value className={styles.value} placeholder={placeholder} />
				<Icon name="chevron-down" />
			</BaseSelect.Trigger>
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
	</Field.Root>
);
