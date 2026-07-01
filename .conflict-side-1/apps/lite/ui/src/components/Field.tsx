import { classes } from "#ui/components/classes.ts";
import styles from "./Field.module.css";
import { ComponentProps, FC, ReactNode } from "react";

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
	<input {...props} className={classes(props.className, "text-13", styles.fieldControl)} />
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
	<div
		className={classes(
			styles.fieldControlWrap,
			iconPosition === "leading" ? styles.fieldIconLeading : styles.fieldIconTrailing,
		)}
	>
		<span className={styles.fieldIcon}>{icon}</span>
		<input {...props} className={classes(props.className, styles.fieldControl)} />
	</div>
);
