import styles from "./ViewHeader.module.css";
import { Button } from "./Button.tsx";
import { classes } from "./classes.ts";
import { Icon } from "./Icon.tsx";
import type { IconName } from "./iconNames.ts";
import { Tooltip } from "./Tooltip.tsx";
import { useId, useState, type ComponentProps, type FC, type ReactNode } from "react";

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
	 * with " · "; the last part is the one that gives way.
	 */
	meta?: ReactNode;
	/**
	 * Tabs that lead the meta line rather than take a toolbar of their own: a landed review's
	 * Diff / Pull Request, on a commit whose header has no other row for them.
	 */
	metaTabs?: ReactNode;
	/**
	 * The rest of what the title begins, as a commit's message body. It stays folded behind a
	 * toggle after the title, and opens under it, above the meta line.
	 */
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
	metaTabs,
	children,
	toolbar,
	actions,
	...props
}) => {
	const actionsNode = present(actions) && <div className={styles.actions}>{actions}</div>;
	const [bodyOpen, setBodyOpen] = useState(false);
	const bodyId = useId();
	const hasBody = present(children);
	const bodyLabel = bodyOpen ? "Hide the full message" : "Show the full message";

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
						{hasBody && (
							<Tooltip content={bodyLabel}>
								<Button
									variant={bodyOpen ? "gray" : "outline"}
									iconOnly
									size="small"
									aria-controls={bodyId}
									aria-expanded={bodyOpen}
									aria-label={bodyLabel}
									className={styles.bodyToggle}
									onClick={() => setBodyOpen(!bodyOpen)}
								>
									<Icon name="kebab" />
								</Button>
							</Tooltip>
						)}
					</div>

					{/* Always there once there is a body, so it can slide open: what is under the header moves
					    down with it rather than jumping a whole message in one frame. */}
					{hasBody && (
						<div
							id={bodyId}
							className={styles.body}
							data-open={bodyOpen || undefined}
							inert={!bodyOpen}
						>
							<div>{children}</div>
						</div>
					)}

					{(present(meta) || present(metaTabs)) && (
						<div className={classes("text-13", styles.meta)}>
							{present(metaTabs) && <div className={styles.metaTabs}>{metaTabs}</div>}
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
