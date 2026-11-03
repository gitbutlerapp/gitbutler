import card from "./card.module.css";
import { classes } from "./classes.ts";
import styles from "./RowGroup.module.css";
import type { ComponentProps, FC } from "react";

/**
 * {@link Cell} rows in a card: settings and choices, such as the parts of a modal that ask
 * something. It is the card `DescriptionList` draws, and it draws the line between its rows, so a
 * row has none of its own. For names and their values use `DescriptionList`; under a title, put it
 * in a `Section`.
 *
 * @public
 * @import import { RowGroup } from "@gitbutler/ui-react/RowGroup.tsx";
 */
export const RowGroup: FC<ComponentProps<"div">> = ({ className, ...props }) => (
	<div {...props} className={classes(card.card, styles.group, className)} />
);
