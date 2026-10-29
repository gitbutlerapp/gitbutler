import { getButtonClassName } from "./Button.tsx";
import { classes } from "./classes.ts";
import { FieldLabelStyles, FieldRootStyles } from "./Field.tsx";
import { Icon } from "./Icon.tsx";
import fieldStyles from "./Field.module.css";
import styles from "./NumberField.module.css";
import { Field, NumberField as BaseNumberField } from "@base-ui/react";
import type { CSSProperties, ReactNode } from "react";

/** @public */
export type NumberFieldProps = {
	/**
	 * The field's label, above the control. Omit for a field the surface labels some other way,
	 * and give it an `aria-label` instead.
	 */
	label?: ReactNode;
	/** What the control reads while it holds no value. */
	placeholder?: string;
	/** Omit to leave the field uncontrolled. `null` is an empty field. */
	value?: number | null;
	defaultValue?: number;
	/** Fires on every change: each keystroke, each press of a stepper. */
	onValueChange?: (value: number | null) => void;
	/**
	 * Fires once the user is done: on blur after typing, on release of a stepper, with each arrow
	 * key. For a setting that should be written then rather than on every keystroke.
	 */
	onValueCommitted?: (value: number | null) => void;
	min?: number;
	max?: number;
	/** What one press of a stepper or an arrow key adds. Shift steps by `largeStep`. */
	step?: number;
	largeStep?: number;
	disabled?: boolean;
	/** Identifies the field when a form is submitted. */
	name?: string;
	className?: string;
	style?: CSSProperties;
	"aria-label"?: string;
};

/**
 * A field for one number the user is as likely to nudge as to type: a size, a count, a limit.
 * The input keeps the text field's chrome with the value centred, and a stepper closes each
 * side; the arrow keys step too. Give it `min`, `max` and `step` so the steppers know where to
 * stop, and `onValueCommitted` for a setting that should be written when the user is done.
 *
 * For a number that is one of a few named choices — a preset — reach for {@link Select}.
 *
 * @public
 * @import import { NumberField } from "@gitbutler/ui-react/NumberField.tsx";
 */
export const NumberField = ({
	label,
	placeholder,
	value,
	defaultValue,
	onValueChange,
	onValueCommitted,
	min,
	max,
	step,
	largeStep,
	disabled = false,
	name,
	className,
	style,
	"aria-label": ariaLabel,
}: NumberFieldProps) => (
	<Field.Root render={<FieldRootStyles />} className={className} style={style}>
		{label !== undefined && <Field.Label render={<FieldLabelStyles />}>{label}</Field.Label>}
		<BaseNumberField.Root
			value={value}
			defaultValue={defaultValue}
			onValueChange={onValueChange}
			onValueCommitted={onValueCommitted}
			min={min}
			max={max}
			step={step}
			largeStep={largeStep}
			disabled={disabled}
			name={name}
		>
			<BaseNumberField.Group className={styles.group}>
				<BaseNumberField.Decrement
					aria-label="Decrease"
					className={classes(
						getButtonClassName({ variant: "outline", iconOnly: true }),
						styles.step,
					)}
				>
					<Icon name="minus" />
				</BaseNumberField.Decrement>
				<BaseNumberField.Input
					aria-label={ariaLabel}
					placeholder={placeholder}
					className={classes("text-13", fieldStyles.fieldControl, styles.input)}
				/>
				<BaseNumberField.Increment
					aria-label="Increase"
					className={classes(
						getButtonClassName({ variant: "outline", iconOnly: true }),
						styles.step,
					)}
				>
					<Icon name="plus" />
				</BaseNumberField.Increment>
			</BaseNumberField.Group>
		</BaseNumberField.Root>
	</Field.Root>
);
