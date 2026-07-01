import { classes } from "#ui/components/classes.ts";
import { Checkbox as BaseCheckbox } from "@base-ui/react/checkbox";
import { ComponentProps, FC } from "react";
import styles from "./Checkbox.module.css";

export const Checkbox: FC<Omit<ComponentProps<typeof BaseCheckbox.Root>, "children">> = (p) => (
	<BaseCheckbox.Root
		{...p}
		className={(x) =>
			classes(styles.checkbox, typeof p.className === "function" ? p.className(x) : p.className)
		}
	>
		<BaseCheckbox.Indicator keepMounted className={styles.checkboxIndicator}>
			<svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
				<path
					d="M9 2.5L4.92139 6.74855C4.52783 7.15851 3.87217 7.15851 3.47861 6.74856L1 4.16667"
					stroke="currentColor"
					strokeWidth="1.5"
				/>
			</svg>
		</BaseCheckbox.Indicator>
	</BaseCheckbox.Root>
);
