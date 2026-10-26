import type { ComponentProps, FC } from "react";
import { classes } from "./classes.ts";
import { Icon } from "./Icon.tsx";
import type { IconName } from "./iconNames.ts";
import styles from "./MetaPart.module.css";

/** The colour of a {@link MetaPart}, which says whose move it is. */
export type MetaPartTone = "neutral" | "green" | "amber" | "blue" | "red";

type Props = {
	/** The glyph that leads the part and says what it is about. */
	icon: IconName;
	/** @default "neutral" */
	tone?: MetaPartTone;
	/** Makes the part a link to what it describes, its words underlined in dots. */
	href?: string;
} & Omit<ComponentProps<"a">, "href">;

/**
 * One part of a meta line, as ⚛️ Core's MetaPart draws it: a glyph, then the words, in a header's
 * meta line or a row's where {@link MetaCount} has no type for what the part says, such as "2 PRs
 * · 1 failing", "CLI 0.2.1662 outdated" or "Needs force push". Amber is work waiting on you, blue is
 * work to send, green is an agent at work or a state that is fine, red is something that failed;
 * neutral is the rest.
 *
 * A part that opens what it describes takes `href`, and only then is it underlined, dotted, in its
 * own colour; a part that only explains on hover is a plain part in a `Tooltip`. Wrap the words a
 * narrow pane can do without in {@link MetaWord}, so "2 PRs · 1 failing" reads "2 · 1" there.
 *
 * @import import { MetaPart } from "@gitbutler/ui-react/MetaPart.tsx";
 */
export const MetaPart: FC<Props> = ({ icon, tone = "neutral", href, children, ...props }) => {
	const className = classes(props.className, "text-13", styles.part, styles[tone]);
	const content = (
		<>
			<Icon name={icon} size={14} className={styles.glyph} />
			<span className={styles.words}>{children}</span>
		</>
	);
	return href !== undefined ? (
		<a {...props} href={href} className={classes(className, styles.link)}>
			{content}
		</a>
	) : (
		<span {...(props as ComponentProps<"span">)} className={className}>
			{content}
		</span>
	);
};

/**
 * Words in a meta part that its glyph already says. A pane under 640px wide drops them, so
 * "↑ 2 unpushed" reads "↑ 2" as on a sidebar row; a screen reader still says them, and the part's
 * tooltip keeps the full story. The pane is the nearest size container: the host makes its page
 * pane one with `container-type: inline-size`, and without one the words always show.
 *
 * @import import { MetaWord } from "@gitbutler/ui-react/MetaPart.tsx";
 */
export const MetaWord: FC<ComponentProps<"span">> = (props) => (
	<span {...props} className={classes(props.className, styles.word)} />
);
