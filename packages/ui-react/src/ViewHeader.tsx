import styles from "./ViewHeader.module.css";
import { classes } from "./classes.ts";
import { Icon } from "./Icon.tsx";
import type { IconName } from "./iconNames.ts";
import type { ComponentProps, FC, ReactNode } from "react";

type Props = {
	/**
	 * What the view shows. Its first part ellipsizes rather than pushing the actions off the edge;
	 * anything after it, as a badge, keeps its size.
	 */
	title: ReactNode;
	/** The glyph for what it is: `branch`, `commit`, `folder`. */
	icon?: IconName;
	/**
	 * An `EntityAvatar` (or its picker) ahead of the title and meta, for an entity rather than a view
	 * of code: a machine, a cloud session. A branch or a commit takes an `icon` instead.
	 */
	avatar?: ReactNode;
	/**
	 * Ahead of the title on its line, for the app's own window controls: Lite's sidebar toggle
	 * and the room it leaves for the traffic lights in full-window mode.
	 */
	leading?: ReactNode;
	/**
	 * One line under the title, cut short with an ellipsis rather than wrapped. Parts are joined
	 * with " · ". A landed review's tabs lead it; its last part is the one that gives way.
	 */
	meta?: ReactNode;
	/** Under the title, above the meta: a commit's message body. */
	children?: ReactNode;
	/**
	 * The row under the title block: the view's tabs, a divider, the commit filter. Omit it when
	 * the view has no tabs, and the header is the title block alone.
	 */
	toolbar?: ReactNode;
	/**
	 * Every button the view has: Send, Apply to workspace, previous and next, a menu, and on a
	 * pull request tab its toolbar. They sit at the end of the header's last line, wherever that
	 * is: the toolbar's end when there is a toolbar, otherwise level with the meta line, otherwise
	 * beside the title. Never the diff's own controls; those sit in the bar over the diff.
	 */
	actions?: ReactNode;
} & Omit<ComponentProps<"header">, "title" | "children">;

const present = (node: ReactNode) => node !== undefined && node !== null && node !== false;

/**
 * The header of a details view: what is shown, a line about it, and the controls for it. Layout
 * only; what goes in each slot is the app's. One component for every view, rather than one per
 * view, so the rules that keep the views alike live here: the actions always end the last line,
 * and the meta line never wraps.
 * @import import { ViewHeader } from "@gitbutler/ui-react/ViewHeader.tsx";
 */
export const ViewHeader: FC<Props> = ({
	title,
	icon,
	avatar,
	leading,
	meta,
	children,
	toolbar,
	actions,
	...props
}) => {
	const actionsNode = present(actions) && <div className={styles.actions}>{actions}</div>;

	return (
		<header {...props} className={classes(props.className, styles.header)}>
			<div className={styles.top}>
				{present(leading) && <div className={styles.leading}>{leading}</div>}
				{present(avatar) && <div className={styles.avatar}>{avatar}</div>}

				<div className={styles.main}>
					<div className={styles.titleLine}>
						{icon !== undefined && <Icon name={icon} />}
						<h2 className={classes("text-15", "text-semibold", styles.title)}>
							{typeof title === "string" ? <span>{title}</span> : title}
						</h2>
					</div>

					{children}

					{present(meta) && (
						<div className={classes("text-13", styles.meta)}>
							{typeof meta === "string" ? <span>{meta}</span> : meta}
						</div>
					)}
				</div>

				{!present(toolbar) && actionsNode}
			</div>

			{present(toolbar) && (
				<div className={styles.toolbar}>
					{toolbar}
					{actionsNode}
				</div>
			)}
		</header>
	);
};

/**
 * The rule between groups in the toolbar: the view's tabs, then what narrows them, as the commit
 * filter.
 * @import import { ViewHeaderDivider } from "@gitbutler/ui-react/ViewHeader.tsx";
 */
export const ViewHeaderDivider: FC = () => <div aria-hidden className={styles.divider} />;
