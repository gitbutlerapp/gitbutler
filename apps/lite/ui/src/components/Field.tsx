import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import styles from "./Field.module.css";
import type { ComponentProps, FC, ReactNode } from "react";

/** @public */
export const FieldRootStyles: FC<ComponentProps<"div">> = (props) => (
	<div {...props} className={classes(props.className, styles.fieldRoot)} />
);

/** @public */
export const FieldLabelStyles: FC<ComponentProps<"label">> = (props) => (
	// oxlint-disable-next-line jsx_a11y/label-has-associated-control -- htmlFor is injected by BaseUI's Field.Label at runtime via {...props}
	<label
		{...props}
		className={classes(props.className, "text-12 text-semibold", styles.fieldLabel)}
	/>
);

/** @public */
export const FieldControlStyles: FC<ComponentProps<"input">> = (props) => (
	<FieldFrame disabled={props.disabled}>
		<input {...props} className={classes(props.className, "text-13", styles.fieldControl)} />
	</FieldFrame>
);

/** @public */
export const FieldTextareaStyles: FC<ComponentProps<"textarea">> = (props) => (
	<textarea
		{...props}
		className={classes(
			props.className,
			styles.fieldControl,
			"text-13",
			styles.fieldControlTextarea,
		)}
	/>
);

/** @public */
export const FieldControlWithIcon: FC<
	ComponentProps<"input"> & { icon: ReactNode; iconPosition?: "leading" | "trailing" }
> = ({ icon, iconPosition = "leading", ...props }) => (
	<FieldFrame icon={icon} iconPosition={iconPosition} disabled={props.disabled}>
		<input {...props} className={classes(props.className, styles.fieldControl)} />
	</FieldFrame>
);

/** The input's frame: the icon the field leads or closes with, and the lock it closes with while
 * disabled. Always there, so toggling `disabled` restyles the input rather than remounting it. */
const FieldFrame: FC<{
	icon?: ReactNode;
	iconPosition?: "leading" | "trailing";
	disabled?: boolean;
	children: ReactNode;
}> = ({ icon, iconPosition = "leading", disabled = false, children }) => (
	<span
		className={classes(
			styles.fieldControlWrap,
			icon !== undefined &&
				(iconPosition === "leading" ? styles.fieldIconLeading : styles.fieldIconTrailing),
			disabled && styles.fieldDisabled,
		)}
	>
		{icon !== undefined && <span className={styles.fieldIcon}>{icon}</span>}
		{children}
		{disabled && (
			<Icon name="lock" size={16} className={classes(styles.fieldIcon, styles.fieldLock)} />
		)}
	</span>
);
