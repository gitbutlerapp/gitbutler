import { formatForDisplaySorted } from "#ui/hotkeys.ts";
import styles from "./Kbd.module.css";
import { classes } from "#ui/components/classes.ts";
import type { HotkeySequence } from "@tanstack/react-hotkeys";
import type { FC } from "react";

type Props = {
	// We can't use the `Hotkey` type because it causes type errors in Storybook. 🤷‍♂️
	hotkey: string | HotkeySequence;
	variant?: "button";
	className?: string;
	keyClassName?: string;
};

const formatKeys = (hotkey: string | HotkeySequence): string =>
	typeof hotkey === "string"
		? formatForDisplaySorted(hotkey)
		: hotkey.map(formatForDisplaySorted).join(" ");

export const Kbd: FC<Props> = ({ hotkey, variant, className, keyClassName }) => (
	<span
		className={classes(
			styles.keys,
			variant === "button" && styles.button,
			"text-semibold",
			className,
		)}
	>
		{formatKeys(hotkey)
			.split(" ")
			.map((key, index) => (
				// oxlint-disable-next-line react/no-array-index-key -- This is fine.
				<kbd key={index} className={classes(styles.key, keyClassName)}>
					{key}
				</kbd>
			))}
	</span>
);
