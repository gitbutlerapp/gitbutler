import { formatForDisplaySorted } from "#ui/hotkeys.ts";
import styles from "./Kbd.module.css";
import { HotkeySequence } from "@tanstack/react-hotkeys";
import { FC } from "react";

type Props = {
	// We can't use the `Hotkey` type because it causes type errors in Storybook. 🤷‍♂️
	hotkey: string | HotkeySequence;
};

const formatKeys = (hotkey: string | HotkeySequence): string =>
	typeof hotkey === "string"
		? formatForDisplaySorted(hotkey)
		: hotkey.map(formatForDisplaySorted).join(" ");

export const Kbd: FC<Props> = ({ hotkey }) => (
	<span className={styles.keys}>
		{formatKeys(hotkey)
			.split(" ")
			.map((key, index) => (
				// oxlint-disable-next-line react/no-array-index-key -- This is fine.
				<kbd key={index} className={styles.key}>
					{key}
				</kbd>
			))}
	</span>
);
