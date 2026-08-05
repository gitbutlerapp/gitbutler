import { classes } from "#ui/components/classes.ts";
import { Switch as BaseSwitch } from "@base-ui/react";
import type { ComponentProps, FC } from "react";
import styles from "./Switch.module.css";

export const Switch: FC<Omit<ComponentProps<typeof BaseSwitch.Root>, "children">> = (p) => (
	<BaseSwitch.Root
		{...p}
		className={(x) =>
			classes(styles.switch, typeof p.className === "function" ? p.className(x) : p.className)
		}
	>
		<BaseSwitch.Thumb className={styles.thumb} />
	</BaseSwitch.Root>
);
