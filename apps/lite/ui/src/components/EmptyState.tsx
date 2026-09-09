import styles from "./EmptyState.module.css";
import { classes } from "#ui/components/classes.ts";
import { Illustration } from "#ui/components/Illustration.tsx";
import type { IllustrationName } from "#ui/components/illustrations.ts";
import type { ComponentProps, FC, ReactNode } from "react";

type Props = {
	/** Omit on a surface too short to hold one; the title and actions carry it. */
	illustration?: IllustrationName;
	/**
	 * One short line naming the state. Sentence case, no full stop. Wraps
	 * balanced, so write it as a sentence and let the lines fall where they may.
	 *
	 * Omit where the body line says it all — a picker that found nothing
	 * reports the miss in one line, and a heading over it would only repeat it.
	 */
	title?: string;
	/**
	 * 1–2 short sentences, around 10–25 words. Say what happens next, or report
	 * the live answer — a count, a name, a time — rather than restating the title.
	 */
	description?: ReactNode;
	/** The actions slot: at most two buttons, and never `pop`. */
	children?: ReactNode;
} & Omit<ComponentProps<"div">, "children" | "title">;

/**
 * A surface at rest with nothing in it — the "Empty state" component in
 * ⚛️ Lite Core, whose description carries the rules for what to put in it.
 *
 * For a surface that is empty, not one still loading: that says so in a line
 * where the list would be. A filter that matched nothing takes the block in a
 * panel with room for it, and a line in a short strip; see "Empty states" in
 * `apps/lite/DESIGN.md`.
 */
export const EmptyState: FC<Props> = ({ illustration, title, description, children, ...props }) => (
	<div {...props} className={classes(props.className, styles.empty)}>
		{illustration !== undefined && <Illustration name={illustration} />}

		<div className={styles.body}>
			<div className={styles.lines}>
				{title !== undefined && (
					<p className={classes("text-14", "text-semibold", "text-balance", styles.title)}>
						{title}
					</p>
				)}
				{description !== undefined && (
					<p className={classes("text-13", "text-body", "text-balance", styles.description)}>
						{description}
					</p>
				)}
			</div>

			{
				/* Spelled out rather than a truthiness check: `{cond && <Button/>}`
			    passes `false`, and an actions row with nothing in it would still
			    spend its gap. */
				children !== undefined && children !== null && children !== false && (
					<div className={styles.actions}>{children}</div>
				)
			}
		</div>
	</div>
);
