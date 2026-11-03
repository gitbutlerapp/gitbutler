import { classes } from "./classes.ts";
import styles from "./Section.module.css";
import { useId, type FC, type ReactNode } from "react";

/**
 * A title over one part of a surface that shows several: a modal of settings in its "Network
 * access", "API credential" and "Setup script" parts, each a `DescriptionList` or a `CodeBlock`.
 * The title names the part to a screen reader too.
 *
 * @public
 * @import import { Section } from "@gitbutler/ui-react/Section.tsx";
 */
export const Section: FC<{
	title: ReactNode;
	children: ReactNode;
	className?: string;
}> = ({ title, children, className }) => {
	const titleId = useId();
	return (
		<section aria-labelledby={titleId} className={classes(styles.section, className)}>
			<div id={titleId} className={classes("text-12", "text-semibold", styles.title)}>
				{title}
			</div>
			{children}
		</section>
	);
};
