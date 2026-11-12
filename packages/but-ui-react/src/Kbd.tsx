import { formatForDisplaySorted } from "./formatHotkey.ts";
import styles from "./Kbd.module.css";
import { classes } from "./classes.ts";
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

/**
 * A shortcut, one keycap per key. Modifiers are ordered as Apple's guidelines order them, so a
 * shortcut reads the same here as in the app's native menus.
 *
 * `variant="button"` is a shortcut inside a button: no caps, in the button's own text at 40%, so it
 * reads as a hint rather than a second label.
 *
 * @import import { Kbd } from "@gitbutler/but-ui-react/Kbd.tsx";
 */
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
