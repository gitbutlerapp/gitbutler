import styles from "./ViewHeader.module.css";
import { Button, type ButtonProps } from "./Button.tsx";
import { classes } from "./classes.ts";
import { Icon } from "./Icon.tsx";
import { Tooltip } from "./Tooltip.tsx";
import type { IconName } from "./iconNames.ts";
import { useId, useState, type ComponentProps, type FC, type ReactNode } from "react";

type Props = {
	/**
	 * What the view shows. Its first part ellipsizes rather than pushing the actions off the edge;
	 * anything after it, as a badge, keeps its size.
	 */
	title: ReactNode;
	/**
	 * The glyph for what it is: an `IconName`, `branch`, `commit`, `folder`; or the element that
	 * draws it, 16px, as a repo's own icon that opens a picker.
	 */
	icon?: ReactNode;
	/**
	 * An `EntityAvatar` (or its picker) ahead of the title and meta, for an entity rather than a view
	 * of code: a machine, a cloud session. A branch or a commit takes an `icon` instead.
	 */
	avatar?: ReactNode;
	/**
	 * Ahead of the title on its line, for the app's own window controls: Lite's sidebar toggle
	 * and the room it leaves for the traffic lights in full-window mode. Only the title's line
	 * makes room for it; the meta line under it starts at the header's edge.
	 */
	leading?: ReactNode;
	/**
	 * One line under the title, cut short with an ellipsis rather than wrapped. Its parts stand
	 * 12px apart with nothing between them, each led by its own glyph (a `MetaCount`, a
	 * `CopyableId`), so a part is told from the next by where its glyph starts; the last part is
	 * the one that gives way. Keep it to what needs the user, one fact and an age.
	 */
	meta?: ReactNode;
	/**
	 * Tabs that lead the meta line rather than take a toolbar of their own: a landed review's
	 * Diff / Pull Request, on a commit whose header has no other row for them.
	 */
	metaTabs?: ReactNode;
	/**
	 * The rest of what the title begins, as a commit's message body. It stays folded behind a
	 * small icon button right after the title, and opens under the title, above the meta line.
	 */
	children?: ReactNode;
	/**
	 * The row under the title block: the view's tabs, a divider, the commit filter. Omit it when
	 * the view has no tabs, and the header is the title block alone.
	 */
	toolbar?: ReactNode;
	/**
	 * Every button the view has, at the header's end: Send to, Update, Apply to workspace,
	 * previous and next, and on a pull request tab its own. They stand in the header's top
	 * corner, level with the title. Each is a {@link ViewHeaderButton}; the menu last, ghost and
	 * icon-only. Never the diff's own controls; those sit in the bar over the
	 * diff.
	 */
	actions?: ReactNode;
} & Omit<ComponentProps<"header">, "title" | "children">;

const present = (node: ReactNode) => node !== undefined && node !== null && node !== false;

/**
 * The header of a details view: what is shown, a line about it, and the controls for it. Layout
 * only; what goes in each slot is the app's. One component for every view, rather than one per
 * view, so the rules that keep the views alike live here: the buttons stand at the end, and the
 * meta line never wraps.
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
	const [bodyOpen, setBodyOpen] = useState(false);
	const bodyId = useId();
	const hasBody = present(children);

	return (
		<header {...props} className={classes(props.className, styles.header)}>
			<div className={styles.top}>
				{present(avatar) && <div className={styles.avatar}>{avatar}</div>}

				<div className={styles.main}>
					<div className={styles.titleLine}>
						{present(leading) && <div className={styles.leading}>{leading}</div>}
						<div className={styles.name}>
							{typeof icon === "string" ? <Icon name={icon as IconName} /> : icon}
							<div className={styles.heading}>
								<h2 className={classes("text-16", "text-semibold", styles.title)}>
									{typeof title === "string" ? <span>{title}</span> : title}
								</h2>
								{hasBody && (
									<Tooltip content={bodyOpen ? "Hide full message" : "Full message"}>
										<Button
											variant="ghost"
											size="small"
											iconOnly
											aria-label="Full message"
											data-pressed={bodyOpen || undefined}
											aria-controls={bodyId}
											aria-expanded={bodyOpen}
											onClick={() => setBodyOpen(!bodyOpen)}
										>
											<Icon name="text-block" />
										</Button>
									</Tooltip>
								)}
							</div>
						</div>
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

				{present(actions) && <div className={styles.actions}>{actions}</div>}
			</div>

			{present(toolbar) && <div className={styles.toolbar}>{toolbar}</div>}
		</header>
	);
};

/**
 * A button in a header's `actions`: a regular {@link Button}, the height of the toolbar's
 * controls under it; an icon-only one is square. `outline` for the next
 * step, `ghost` and `iconOnly` for the menu.
 * @import import { ViewHeaderButton } from "@gitbutler/ui-react/ViewHeader.tsx";
 */
export const ViewHeaderButton: FC<Omit<ButtonProps, "size">> = (props) => <Button {...props} />;

/**
 * The dim " · " between two words of a status that have no glyphs to set them apart, as
 * "offline · seen 2m". A header's meta line needs none: its parts lead with their glyphs.
 * @import import { ViewHeaderMetaSeparator } from "@gitbutler/ui-react/ViewHeader.tsx";
 */
export const ViewHeaderMetaSeparator: FC = () => (
	<span aria-hidden className={styles.metaSeparator}>
		·
	</span>
);

/**
 * The rule between groups in the toolbar: the view's tabs, then what narrows them, as the commit
 * filter.
 * @import import { ViewHeaderDivider } from "@gitbutler/ui-react/ViewHeader.tsx";
 */
export const ViewHeaderDivider: FC = () => <div aria-hidden className={styles.divider} />;
