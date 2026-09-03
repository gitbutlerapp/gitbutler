import styles from "./Illustration.module.css";
import { classes } from "#ui/components/classes.ts";
import { illustrations, type IllustrationName } from "./illustrations.ts";
import type { ComponentProps, FC } from "react";

type Props = {
	name: IllustrationName;
	/** Width in px; the height follows from the artwork's own proportions. */
	width?: number;
} & ComponentProps<"i">;

export const Illustration: FC<Props> = ({ name, width, ...props }) => (
	<i
		{...props}
		className={classes(props.className, styles.illustration)}
		data-illustration
		aria-hidden
		style={{
			...props.style,
			...(width !== undefined ? { "--illustration-width": `${width}px` } : undefined),
		}}
		// oxlint-disable-next-line react/no-danger -- SVGs are bundled app assets.
		dangerouslySetInnerHTML={{ __html: illustrations[name] }}
	/>
);
