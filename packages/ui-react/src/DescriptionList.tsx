import card from "./card.module.css";
import { classes } from "./classes.ts";
import { Code } from "./Code.tsx";
import { CopyButton } from "./copy-button.tsx";
import styles from "./DescriptionList.module.css";
import type { ComponentProps, FC, ReactNode } from "react";

/**
 * Names and their values in a card, one pair to a row: the settings to type into another app's
 * form, the facts about a thing. It reads, it doesn't edit; for a form use `Field`, and for a list
 * of things rather than of names and values use `List`. Under a title, put it in a `Section`.
 *
 * @public
 * @import import { DescriptionList, DescriptionListItem } from "@gitbutler/ui-react/DescriptionList.tsx";
 */
export const DescriptionList: FC<ComponentProps<"dl">> = ({ className, ...props }) => (
	<dl {...props} className={classes("text-13", card.card, styles.list, className)} />
);

/**
 * One row of a {@link DescriptionList}: the name on the left, its value beside it. The value can be
 * anything, a button that makes it included.
 *
 * With `onCopy` the value is text to paste somewhere else, so it is shown as `Code`, as it will be
 * typed, with the copy button `CodeBlock` has after it.
 *
 * With `code` the name is a flag or a key rather than a word, so it is shown as `Code` too.
 *
 * @public
 * @import import { DescriptionList, DescriptionListItem } from "@gitbutler/ui-react/DescriptionList.tsx";
 */
export const DescriptionListItem: FC<
	{ label: string; code?: boolean } & (
		| { children: ReactNode; onCopy?: undefined }
		| { children: string; onCopy: (value: string) => void }
	)
> = (props) => (
	<div className={styles.item}>
		<dt className={classes(styles.label, props.code && styles.codeLabel)}>
			{props.code ? <Code>{props.label}</Code> : props.label}
		</dt>
		<dd className={styles.value}>
			{props.onCopy === undefined ? (
				props.children
			) : (
				<>
					<span className={styles.literal}>
						<Code>{props.children}</Code>
					</span>
					<CopyButton
						label={`Copy ${lowerFirst(props.label)}`}
						onCopy={() => props.onCopy(props.children)}
						className={styles.copy}
					/>
				</>
			)}
		</dd>
	</div>
);

/** "Allowed domains" reads "Copy allowed domains"; "API key" keeps its capitals. */
const lowerFirst = (label: string) =>
	label.replace(/^[A-Z](?=[a-z])/, (letter) => letter.toLowerCase());
