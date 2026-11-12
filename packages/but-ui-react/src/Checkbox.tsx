import { classes } from "./classes.ts";
import { Checkbox as BaseCheckbox } from "@base-ui/react";
import type { ComponentProps, FC } from "react";
import styles from "./Checkbox.module.css";

/**
 * A box for one choice among many that don't exclude each other, or for a row that can be picked
 * alongside others.
 *
 * Indeterminate is filled like a checked box rather than drawn as an emptier third state: it stands
 * for a folder with some of its files checked, so it belongs on the checked side. The ground behind
 * the box is `--checkbox-bg`, so a row can sit it on its own background rather than the popup's.
 *
 * @import import { Checkbox } from "@gitbutler/but-ui-react/Checkbox.tsx";
 */
export const Checkbox: FC<Omit<ComponentProps<typeof BaseCheckbox.Root>, "children">> = (p) => (
	<BaseCheckbox.Root
		{...p}
		className={(x) =>
			classes(styles.checkbox, typeof p.className === "function" ? p.className(x) : p.className)
		}
	>
		<BaseCheckbox.Indicator keepMounted className={styles.checkboxIndicator}>
			{/* Both glyphs are always drawn and CSS shows one, so a checkbox that
			    changes state doesn't remount its indicator mid-transition. */}
			<svg
				className={styles.checkboxTick}
				width="10"
				height="10"
				viewBox="0 0 10 10"
				fill="none"
				aria-hidden="true"
			>
				<path
					d="M9 2.5L4.92139 6.74855C4.52783 7.15851 3.87217 7.15851 3.47861 6.74856L1 4.16667"
					stroke="currentColor"
					strokeWidth="1.5"
				/>
			</svg>
			<svg
				className={styles.checkboxDash}
				width="10"
				height="10"
				viewBox="0 0 10 10"
				fill="none"
				aria-hidden="true"
			>
				<path d="M1.5 5H8.5" stroke="currentColor" strokeWidth="1.5" />
			</svg>
		</BaseCheckbox.Indicator>
	</BaseCheckbox.Root>
);
