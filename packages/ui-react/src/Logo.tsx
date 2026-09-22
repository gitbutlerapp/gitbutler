import styles from "./Logo.module.css";
import { classes } from "./classes.ts";
import { assert } from "./assert.ts";
import type { ComponentProps, FC } from "react";

/** Brand marks, kept apart from `icons` because they carry their own colours. */
const modules = import.meta.glob<string>("./logos/*.svg", {
	query: "?raw",
	import: "default",
	eager: true,
});

const logos = new Map<string, string>();
for (const [path, svg] of Object.entries(modules)) {
	const name = path.replace(/^.*\/(.+)\.svg$/, "$1");
	logos.set(name, svg);
}

export type LogoName = "github" | "gitlab" | "bitbucket";

type Props = {
	name: LogoName;
	/** The mark as a silhouette on the recessed ground: a forge the user has not connected. */
	muted?: boolean;
} & ComponentProps<"i">;

export const Logo: FC<Props> = ({ name, muted = false, ...props }) => (
	<i
		{...props}
		className={classes(props.className, styles.logo)}
		aria-hidden
		// oxlint-disable-next-line react/no-danger -- SVGs are bundled app assets.
		dangerouslySetInnerHTML={{ __html: assert(logos.get(muted ? `${name}-muted` : name)) }}
	/>
);
