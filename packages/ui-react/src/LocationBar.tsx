import type { ComponentProps, FC, ReactNode } from "react";
import { classes } from "./classes.ts";
import styles from "./LocationBar.module.css";

type Props = {
	/**
	 * The window's own controls before the trail: the sidebar toggle, and Back while the sidebar is
	 * hidden, with a {@link LocationBarDivider} between them. Ghost, regular, icon-only buttons: the
	 * bar sizes them to 24×22.
	 */
	leading?: ReactNode;
	/**
	 * After the trail, at the bar's end: the state of what the page shows, as whether its machine
	 * is live, and the button that refreshes it, a ghost, regular, icon-only button like the leading
	 * ones.
	 */
	state?: ReactNode;
	/** Where the page sits: its `Breadcrumbs`. */
	children?: ReactNode;
} & Omit<ComponentProps<"div">, "children">;

const present = (node: ReactNode) => node !== undefined && node !== null && node !== false;

/**
 * The bar above a view's header: the window's controls, where the page sits, then the state of
 * what it shows. Layout only; what goes in each part is the app's. The trail gives way before the
 * state does: it folds and ellipsizes, the state keeps its size.
 *
 * @import import { LocationBar, LocationBarDivider } from "@gitbutler/ui-react/LocationBar.tsx";
 */
export const LocationBar: FC<Props> = ({ leading, state, children, className, ...props }) => (
	<div {...props} className={classes(className, styles.bar)}>
		{present(leading) && (
			<>
				<div className={styles.leading}>{leading}</div>
				<LocationBarDivider />
			</>
		)}
		<div className={styles.trail}>{children}</div>
		{present(state) && <div className={classes("text-13", styles.state)}>{state}</div>}
	</div>
);

/**
 * The hairline between the bar's controls, and between them and the trail.
 * @import import { LocationBarDivider } from "@gitbutler/ui-react/LocationBar.tsx";
 */
export const LocationBarDivider: FC = () => <div aria-hidden className={styles.divider} />;
