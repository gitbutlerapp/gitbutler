import styles from "./Code.module.css";
import { classes } from "./classes.ts";
import type { ComponentProps, FC } from "react";

/**
 * Code in running text: a command to run, a flag, a name to type. It reads as Markdown's inline
 * code does, so a command in an empty state or a hint looks the same as one in a description.
 * For a command the user is meant to take away, offer a copy button beside the text as well.
 *
 * @public
 * @import import { Code } from "@gitbutler/ui-react/Code.tsx";
 */
export const Code: FC<ComponentProps<"code">> = (props) => (
	<code {...props} className={classes(props.className, styles.code)} />
);
