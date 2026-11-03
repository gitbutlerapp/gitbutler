import { mergeProps, useRender } from "@base-ui/react";
import type { FC, ReactNode } from "react";
import styles from "./Cell.module.css";
import { classes } from "./classes.ts";
import { Icon } from "./Icon.tsx";
import type { IconName } from "./iconNames.ts";
import { StatusLed } from "./StatusLed.tsx";

/** A slot the host filled with nothing (`null`, `false`) is no slot. */
const filled = (slot: ReactNode) => (slot === null || slot === false ? undefined : slot);

/**
 * One row: an optional `leading` picture or checkbox, a `title` with its `icon`, light (`online`)
 * and `badge`, an optional `description` and `meta`, and what ends the row (`trailing`). For list
 * items, card rows, machines, repositories and settings; several go in a {@link RowGroup}, which
 * draws the lines between them.
 *
 * `trailing` sits on the title's line: `"chevron"` on a row that opens something, or the control
 * itself (a `Switch`, a tick, a `Button`). Leave it out on a plain row.
 *
 * A row that does something is the whole click target, through `render`: an `<a href>` or a
 * `<button>` for a row that opens something, a `<label htmlFor>` the trailing `Switch`'s `id` for
 * a setting, so a click anywhere on the row flips it. Then it tints under the pointer and draws the
 * focus ring, the label's while its switch has keyboard focus. A row that renders as a `<div>`, the
 * default, is static: a control in it answers on its own.
 *
 * `disabled` fades the row to half on the grey ground and stops it answering the pointer; it
 * doesn't disable what the row renders, so pass `disabled` to the `<button>` or the `Switch` too,
 * or leave out the link's `href`. A disabled control inside doesn't fade a second time. Say why in
 * the `description`.
 *
 * @public
 * @import import { Cell } from "@gitbutler/ui-react/Cell.tsx";
 */
export const Cell: FC<
	{
		title: ReactNode;
		/** A 16px glyph before the title, in the quiet grey. */
		icon?: IconName;
		/** Gives the title a machine's light: lit while it is connected. Leave it out on anything
		 * that isn't a machine. */
		online?: boolean;
		/** A `Badge`, at the end of the title's line. */
		badge?: ReactNode;
		/** One or two sentences under the title, in grey: what the row does, or why it can't. */
		description?: ReactNode;
		/** Facts under the description, in a wrapping line of 13px grey: an author, an age, a hash.
		 * Their icons are 14px unless told. */
		meta?: ReactNode;
		/** What leads the row, at its own size and in grey, at the top: a machine's 28px glyph, a
		 * `Checkbox`. */
		leading?: ReactNode;
		/** What ends the row, on the title's line: `"chevron"` for a row that opens something, or
		 * a control. */
		// oxlint-disable-next-line typescript/no-redundant-type-constituents -- Names the one string the row draws itself, as ⚛️ Core's trailing=chevron; any other node is the control.
		trailing?: "chevron" | ReactNode;
		disabled?: boolean;
	} & Omit<useRender.ComponentProps<"div">, "title">
> = ({
	title,
	icon,
	online,
	badge: badgeSlot,
	description: descriptionSlot,
	meta: metaSlot,
	leading: leadingSlot,
	trailing: trailingSlot,
	disabled = false,
	render,
	...props
}) => {
	const [badge, description, meta, leading, trailing] = [
		filled(badgeSlot),
		filled(descriptionSlot),
		filled(metaSlot),
		filled(leadingSlot),
		filled(trailingSlot),
	];
	return useRender({
		render: render ?? <div />,
		state: { disabled },
		props: mergeProps<"div">(props, {
			className: styles.cell,
			children: (
				<>
					{leading !== undefined && <span className={styles.leading}>{leading}</span>}
					<span className={styles.text}>
						<span className={styles.titleLine}>
							{icon !== undefined && <Icon name={icon} className={styles.icon} />}
							<span className={styles.label}>
								<span className={classes("text-14", "text-semibold", styles.title)}>{title}</span>
								{online !== undefined && <StatusLed online={online} />}
							</span>
							{badge !== undefined && <span className={styles.badge}>{badge}</span>}
							{
								// On the title's line itself rather than in the trailing column, as ⚛️ Core draws
								// it: a step after the badge, with the description free to run under it.
								trailing === "chevron" && <Icon name="chevron-right" className={styles.icon} />
							}
						</span>
						{description !== undefined && (
							<span className={classes("text-13", "text-body", styles.description)}>
								{description}
							</span>
						)}
						{meta !== undefined && <span className={classes("text-13", styles.meta)}>{meta}</span>}
					</span>
					{trailing !== undefined && trailing !== "chevron" && (
						<span className={styles.trailing}>{trailing}</span>
					)}
				</>
			),
		}),
	});
};
