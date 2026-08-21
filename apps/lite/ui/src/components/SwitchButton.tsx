import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { Switch } from "#ui/components/Switch.tsx";
import { type ComponentProps, type FC, type ReactNode, useId } from "react";
import styles from "./SwitchButton.module.css";

/**
 * The Button variants this borrows chrome from.
 *
 * @public
 */
export type SwitchButtonVariant = "ghost" | "outline";

type Props = Omit<ComponentProps<typeof Switch>, "className" | "id"> & {
	/** Sits beside the switch and shares its hit area. */
	label: ReactNode;
	variant?: SwitchButtonVariant;
	className?: string;
};

/**
 * A switch and its label wearing button chrome, so the pair reads and behaves
 * as one control rather than a toggle with text next to it.
 *
 * The chrome lives on a `<label>` rather than around the switch, so the text
 * shares the switch's hit area: base-ui points the label at the hidden
 * checkbox it renders alongside the `<span role="switch">`.
 */
export const SwitchButton: FC<Props> = ({
	label,
	variant = "ghost",
	className,
	...switchProps
}) => {
	const id = useId();

	return (
		<label
			className={classes(
				getButtonClassName({ variant, size: "regular" }),
				styles.switchButton,
				className,
			)}
			htmlFor={id}
		>
			<Switch {...switchProps} id={id} />
			{label}
		</label>
	);
};
