import { classes } from "#ui/components/classes.ts";
import { Switch as BaseSwitch } from "@base-ui/react";
import type { ComponentProps, FC } from "react";
import styles from "./Switch.module.css";

/** @public */
export type SwitchSize =
	/** 24×14, for a switch inside a row of 13px text — a list, a toolbar. */
	| "default"
	/** 28×16, for a switch that is a settings row's control beside its 15px title. */
	| "large";

/**
 * An on/off switch for a setting that takes effect immediately. Use it where there is no Save
 * to press; for a choice confirmed later, use a Checkbox. Pairing a switch with its own label is
 * {@link SwitchButton}'s job, not this one's.
 *
 * @public
 */
export const Switch: FC<
	{ size?: SwitchSize } & Omit<ComponentProps<typeof BaseSwitch.Root>, "children">
> = ({ size = "default", ...p }) => (
	<BaseSwitch.Root
		{...p}
		className={(x) =>
			classes(
				styles.switch,
				size === "large" && styles.large,
				typeof p.className === "function" ? p.className(x) : p.className,
			)
		}
	>
		<BaseSwitch.Thumb className={styles.thumb} />
	</BaseSwitch.Root>
);
