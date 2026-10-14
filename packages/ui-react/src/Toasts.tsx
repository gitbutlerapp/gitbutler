import { Toast } from "@base-ui/react";
import type { FC } from "react";
import { classes } from "./classes.ts";
import { Button } from "./Button.tsx";
import { Icon } from "./Icon.tsx";
import type { IconName } from "./iconNames.ts";
import styles from "./Toasts.module.css";
import popupStyles from "./Popup.module.css";

/**
 * What a toast's `type` says about how the news landed, passed to `add({ type })`. Base UI takes any
 * string; one it doesn't name here, or none, reads as `info`.
 *
 * @public
 */
export type ToastType =
	/** Something happened worth saying, with no verdict attached. */
	| "info"
	/** An act that came off only in part: some changes were left behind. */
	| "warning"
	/** An act that failed, or refused to run. */
	| "error"
	/** An act that came off, where nothing else on screen shows it. */
	| "success";

const glyphs: Record<ToastType, IconName> = {
	info: "info",
	warning: "warning",
	error: "danger",
	success: "tick-circle",
};

/**
 * What a toast carries in Base UI's `data` beyond its own fields: `add({ data: { icon } })`.
 *
 * @public
 */
export type ToastData = {
	/** Overrides the type's own glyph; the type still colours it. */
	icon?: IconName;
};

const glyph = (type: string | undefined): IconName =>
	type !== undefined && Object.hasOwn(glyphs, type) ? glyphs[type as ToastType] : glyphs.info;

/**
 * Where toasts appear. Render it once inside Base UI's `Toast.Provider`; anything under the
 * provider raises one through `Toast.useToastManager().add({ type, title, description, actionProps })`.
 *
 * Each toast is a card in the window's bottom-right corner: a glyph, a title naming what happened, a
 * description that can hold real content, at most one action under it, and a close button. Every
 * type wears the same surface; the glyph alone carries the verdict.
 *
 * @import import { Toasts } from "@gitbutler/ui-react/Toasts.tsx";
 */
export const Toasts: FC = () => {
	const { toasts } = Toast.useToastManager<ToastData>();

	return (
		<Toast.Portal>
			<Toast.Viewport className={styles.viewport}>
				{toasts.map((toast, index) => (
					<Toast.Root
						key={toast.id}
						toast={toast}
						className={styles.root}
						// A leaving toast keeps its place in the list but not its visible index, so this
						// is how the stylesheet tells the front one, which slides out, from the rest.
						data-front={index === 0 || undefined}
					>
						{/* The root stacks and moves; the surface inside it is what's drawn, so the root's
						    hover bridge isn't clipped by the popup's edge. */}
						<div className={classes(popupStyles.popup, styles.surface)}>
							{/* The glyph and the close button stay outside Toast.Content: it watches its
							    subtree for mutations and re-measures synchronously on each one. */}
							<div className={styles.row}>
								<Icon name={toast.data?.icon ?? glyph(toast.type)} className={styles.icon} />
								<Toast.Content className={styles.content}>
									<div className={styles.text}>
										<Toast.Title render={<strong />} className="text-14 text-semibold" />
										<Toast.Description
											render={
												// Default is `p` which restricts content elements.
												<div />
											}
											className={classes(styles.description, "text-13", "text-body")}
										/>
									</div>
									{toast.actionProps && (
										<Toast.Action render={<Button />} className={styles.action} />
									)}
								</Toast.Content>
								<div aria-hidden className={styles.divider} />
								<Toast.Close
									aria-label="Dismiss"
									render={<Button variant="ghost" size="small" iconOnly />}
									className={styles.dismiss}
								>
									<Icon name="cross" />
								</Toast.Close>
							</div>
						</div>
					</Toast.Root>
				))}
			</Toast.Viewport>
		</Toast.Portal>
	);
};
