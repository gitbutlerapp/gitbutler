import { classes } from "./classes.ts";
import { Icon } from "./Icon.tsx";
import type { IconName } from "./iconNames.ts";
import styles from "./Banner.module.css";
import { Match } from "effect";
import type { ComponentProps, FC, ReactNode } from "react";

/**
 * Chosen by meaning, as Badge's are: the colour says what the condition is, not how loud it is.
 *
 * @public
 */
export type BannerVariant =
	/** Something is degraded but still usable: what the user sees may be stale. */
	| "warn"
	/** Something failed, and the surface can't do its job until it is fixed. */
	| "danger"
	/** Something new the user can act on. The accent, so at most one per surface. */
	| "pop"
	/** A condition worth confirming in place. Rarely needed: most good news is a Snackbar. */
	| "safe"
	/** A neutral note with no verdict. */
	| "gray";

/** The glyph a variant leads with when the caller names none of its own. */
const defaultIcon = (variant: BannerVariant): IconName =>
	Match.value(variant).pipe(
		Match.when("warn", () => "warning" as const),
		Match.when("danger", () => "danger" as const),
		Match.when("pop", () => "info" as const),
		Match.when("safe", () => "tick-circle" as const),
		Match.when("gray", () => "info" as const),
		Match.exhaustive,
	);

/**
 * A condition that holds right now, set into the layout above the thing it affects: live updates
 * paused, a machine offline, a server that won't answer. It stays while the condition is true and
 * leaves when it clears, so it has no close button; the caller renders it only while it applies.
 *
 * The title is the one line every banner has. `children` is an optional second sentence under it.
 * The icon and the action line up with the title. The action is the fix (Retry, Reconnect, Pull):
 * one small ghost `Button`, which takes the banner's colour.
 *
 * Use a Snackbar for the outcome of something the user just did; a banner is for state.
 *
 * @public
 * @import import { Banner } from "@gitbutler/ui-react/Banner.tsx";
 */
export const Banner: FC<
	{
		variant: BannerVariant;
		/** One line saying the condition. */
		title: ReactNode;
		/** Overrides the variant's own glyph; `false` drops it. */
		icon?: IconName | false;
		/** One small ghost `Button`. */
		action?: ReactNode;
	} & Omit<ComponentProps<"div">, "title">
> = ({ variant, title, icon, action, children, ...props }) => (
	<div
		// A failure interrupts; the rest is there to be read whenever the reader gets to it.
		role={variant === "danger" ? "alert" : "status"}
		{...props}
		className={classes(
			props.className,
			styles.banner,
			Match.value(variant).pipe(
				Match.when("warn", () => styles.warn),
				Match.when("danger", () => styles.danger),
				Match.when("pop", () => styles.pop),
				Match.when("safe", () => styles.safe),
				Match.when("gray", () => styles.gray),
				Match.exhaustive,
			),
		)}
	>
		{icon !== false && <Icon name={icon ?? defaultIcon(variant)} className={styles.icon} />}
		<div className={styles.content}>
			<div className={classes(styles.title, "text-13", "text-semibold")}>{title}</div>
			{children !== undefined && (
				<div className={classes(styles.description, "text-12", "text-body")}>{children}</div>
			)}
		</div>
		{action !== undefined && <div className={styles.action}>{action}</div>}
	</div>
);
