import { classes } from "./classes.ts";
import { Radio as BaseRadio } from "@base-ui/react";
import type { ComponentProps, FC } from "react";
import styles from "./Radio.module.css";

/**
 * One choice among several that exclude each other. Radios go in Base UI's `RadioGroup`, which
 * holds the value; each radio takes the `value` it stands for.
 *
 * Reach for one rarely: two or three short options read better as a `ToggleGroup`, and a long
 * list as a popup with a tick on the current choice.
 *
 * @import import { Radio } from "@gitbutler/ui-react/Radio.tsx";
 */
export const Radio: FC<Omit<ComponentProps<typeof BaseRadio.Root>, "children">> = (p) => (
	<BaseRadio.Root
		{...p}
		className={(x) =>
			classes(styles.radio, typeof p.className === "function" ? p.className(x) : p.className)
		}
	>
		<BaseRadio.Indicator keepMounted className={styles.radioIndicator} />
	</BaseRadio.Root>
);
