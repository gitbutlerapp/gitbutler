import { classes } from "#ui/components/classes.ts";
import { openLinkExternally } from "#ui/external-link.ts";
import type { ComponentProps, FC } from "react";
import styles from "./TextLink.module.css";

type Props = Omit<ComponentProps<"a">, "href"> & {
	href: string;
};

/**
 * A link that reads as a link: underlined text with an arrow hung off the
 * end, since every link leaves the app. The arrow is drawn here rather than
 * typed, because the text fonts don't carry ↗ at every weight. Opens in the
 * system browser unless an `onClick` takes over.
 */
export const TextLink: FC<Props> = ({
	href,
	children,
	className,
	onClick = openLinkExternally,
	...props
}) => (
	<a {...props} href={href} className={classes(styles.link, className)} onClick={onClick}>
		{children}
		<svg aria-hidden className={styles.arrow} viewBox="0 0 12 12" fill="none">
			<path
				d="M0.5 11.5L11.5 0.5M11.5 10V0.5H2"
				stroke="currentColor"
				vectorEffect="non-scaling-stroke"
			/>
		</svg>
	</a>
);
