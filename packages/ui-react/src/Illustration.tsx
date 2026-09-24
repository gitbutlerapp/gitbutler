import styles from "./Illustration.module.css";
import { classes } from "./classes.ts";
import { illustrations, type IllustrationName } from "./illustrations.ts";
import type { ComponentProps, FC } from "react";

type Props = {
	name: IllustrationName;
} & ComponentProps<"i">;

/**
 * A drawing at the size it was drawn, never scaled. Pick it by meaning: `cactus` a list with
 * nothing in it, `papers` a search or filter that found nothing or a state the app can't name, `id-card` signing in, `terminal` the command line, `waving` all good with nothing to do —
 * and `waving` only in a large view, never a sidebar or a popup. See "Illustrations" in
 * `packages/ui-react/DESIGN.md`.
 *
 * @import import { Illustration } from "@gitbutler/ui-react/Illustration.tsx";
 */
export const Illustration: FC<Props> = ({ name, ...props }) => (
	<i
		{...props}
		className={classes(props.className, styles.illustration)}
		data-illustration
		aria-hidden
		// oxlint-disable-next-line react/no-danger -- SVGs are bundled app assets.
		dangerouslySetInnerHTML={{ __html: illustrations[name] }}
	/>
);
