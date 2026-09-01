import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import type { IconName } from "#ui/components/iconNames.ts";
import { Kbd } from "#ui/components/Kbd.tsx";
import styles from "./Popup.module.css";
import { AlertDialog, Dialog, mergeProps, Popover, useRender } from "@base-ui/react";
import type { HotkeySequence } from "@tanstack/react-hotkeys";
import {
	useEffect,
	useState,
	type ComponentProps,
	type FC,
	type ReactElement,
	type ReactNode,
} from "react";

/**
 * The container every overlay in Lite is built from: modals, dropdowns and the toolbox all wear
 * this one surface. Use it directly only for something that floats without opening — a toolbox, a
 * hover card. Anything that opens over the app wants {@link Modal} or {@link Dropdown}, which wrap
 * this in the Base UI primitive carrying the focus and dismissal behaviour.
 *
 * Pass `render` where the popup has to be a list primitive's own part — a combobox owns its popup
 * and cannot be handed one. `anchored` is what {@link Dropdown} adds over {@link Modal}: it grows
 * out of the corner it is anchored to rather than fading in place.
 *
 * @public
 */
export const Popup: FC<{ anchored?: boolean } & useRender.ComponentProps<"div">> = ({
	anchored = false,
	render,
	...props
}) =>
	useRender({
		render: render ?? <div />,
		props: mergeProps<"div">(props, {
			className: classes(styles.popup, anchored && styles.dropdown),
		}),
	});

/**
 * How wide a modal opens. The steps are the three widths the app already uses — a prompt, a
 * settings-sized pane, and a working surface — rather than a free measurement per caller.
 *
 * @public
 */
export type ModalSize = "small" | "medium" | "large";

const sizeClassName = (size: ModalSize): string | undefined =>
	size === "small" ? styles.sizeSmall : size === "medium" ? styles.sizeMedium : styles.sizeLarge;

/** @public */
export type ModalProps = {
	/** Omit to leave the modal uncontrolled and drive it from `trigger` alone. */
	open?: boolean;
	onOpenChange?: (open: boolean) => void;
	/** Rendered as the modal's trigger. Omit for a modal opened from elsewhere in the app. */
	trigger?: ReactElement;
	/**
	 * Takes the `alertdialog` role and refuses to close on Escape or a backdrop click: the question
	 * has to be answered rather than dismissed. For credentials and destructive confirmations.
	 *
	 * @default false
	 */
	alert?: boolean;
	/** @default "medium" */
	size?: ModalSize;
	/**
	 * Where the modal sits in the window. A picker opens `top`, so its list grows downward the way a
	 * command palette does; everything else centres.
	 *
	 * @default "center"
	 */
	align?: "center" | "top";
	/**
	 * Recesses the modal's own ground so the groups inside it read as cards sitting on it, rather
	 * than as sections of one sheet. For a pane holding more than one group — settings, a resolver.
	 *
	 * @default false
	 */
	recessed?: boolean;
	/** What takes focus as the modal opens, when the first tabbable element is not the right one. */
	initialFocus?: ComponentProps<typeof Dialog.Popup>["initialFocus"];
} & ComponentProps<"div">;

/**
 * A popup that opens over the whole window, dimming what it covers. The backdrop is the only thing
 * separating it from a {@link Dropdown} — the container beneath is the same.
 *
 * The modal owns its chrome and placement, not its insides: a confirmation stacks its own prompt
 * and buttons, a picker fills itself with {@link PopupSearch} and {@link PopupSection}, and a pane
 * as involved as settings lays itself out entirely.
 *
 * @public
 */
export const Modal: FC<ModalProps> = ({
	open,
	onOpenChange,
	trigger,
	alert = false,
	size = "medium",
	align = "center",
	recessed = false,
	initialFocus,
	children,
	...props
}) => {
	// AlertDialog re-exports Dialog's portal, backdrop, viewport and popup unchanged — only the root
	// and the trigger carry the different role and dismissal behaviour.
	const Root = alert ? AlertDialog.Root : Dialog.Root;
	const Trigger = alert ? AlertDialog.Trigger : Dialog.Trigger;

	// Base UI only plays the open animation when `open` turns true *after* mount, and the app mounts
	// its modals already open — `{shown && <Picker open />}` in the workspace page. Holding the
	// first frame closed gives Base UI the change it animates from. A modal that was already
	// mounted when `open` flipped is past this by then and is unaffected.
	const [pastFirstFrame, setPastFirstFrame] = useState(false);
	useEffect(() => {
		const frame = requestAnimationFrame(() => setPastFirstFrame(true));
		return () => cancelAnimationFrame(frame);
	}, []);

	return (
		<Root
			open={open === undefined ? undefined : open && pastFirstFrame}
			onOpenChange={onOpenChange}
		>
			{trigger !== undefined && <Trigger render={trigger} />}
			<Dialog.Portal>
				<Dialog.Backdrop className={styles.backdrop} />
				<Dialog.Viewport
					className={classes(
						styles.viewport,
						align === "top" ? styles.viewportTop : styles.viewportCenter,
					)}
				>
					<Popup
						{...props}
						className={classes(
							props.className,
							styles.modal,
							sizeClassName(size),
							recessed && styles.recessed,
						)}
						render={<Dialog.Popup initialFocus={initialFocus} />}
					>
						{children}
					</Popup>
				</Dialog.Viewport>
			</Dialog.Portal>
		</Root>
	);
};

/** @public */
export type DropdownProps = {
	/** Omit to leave the dropdown uncontrolled and drive it from `trigger` alone. */
	open?: boolean;
	onOpenChange?: (open: boolean) => void;
	/** Rendered as the dropdown's trigger, and what it anchors to. */
	trigger: ReactElement;
	/** @default "bottom" */
	side?: "top" | "bottom" | "left" | "right";
	/** @default "start" */
	align?: "start" | "center" | "end";
	/** @default 4 */
	sideOffset?: number;
} & ComponentProps<"div">;

/**
 * A popup anchored to the control that opened it, with nothing dimmed behind it. Wears the same
 * container as {@link Modal}, minus the backdrop.
 *
 * This is for anchored *panels* — a notification list, a reaction picker, a filter. Menus in Lite
 * are Electron's own, raised through `native-menu.ts`; a dropdown is not the place to rebuild one.
 *
 * @public
 */
export const Dropdown: FC<DropdownProps> = ({
	open,
	onOpenChange,
	trigger,
	side = "bottom",
	align = "start",
	sideOffset = 4,
	children,
	...props
}) => (
	<Popover.Root open={open} onOpenChange={onOpenChange}>
		<Popover.Trigger render={trigger} />
		<Popover.Portal>
			<Popover.Positioner side={side} align={align} sideOffset={sideOffset}>
				<Popup anchored {...props} render={<Popover.Popup />}>
					{children}
				</Popup>
			</Popover.Positioner>
		</Popover.Portal>
	</Popover.Root>
);

/**
 * The filter row a popup leads with. Full-bleed against the container's top edge, divided from the
 * list below by a hairline rather than fenced off in a field of its own — the popup is already the
 * box the query sits in.
 *
 * A plain input by default. Pass `render` when the query drives a list primitive and the input has
 * to be that primitive's own — a picker's `Autocomplete.Input`, say.
 *
 * Pass `onClear` while there is a query to clear: the row's trailing glyph becomes a button that
 * empties the field. The row does not hold the query, so whether there is anything to clear is the
 * caller's to say — leave `onClear` out and the row shows the magnifier it searches under.
 *
 * @public
 */
export const PopupSearch: FC<{ onClear?: () => void } & useRender.ComponentProps<"input">> = ({
	render,
	onClear,
	...props
}) => {
	const input = useRender({
		render: render ?? <input />,
		props: mergeProps<"input">(props, {
			className: classes("text-13", styles.searchInput),
		}),
	});

	return (
		<div className={styles.search}>
			{input}
			{onClear === undefined ? (
				<Icon name="search" className={styles.searchIcon} />
			) : (
				<button type="button" className={styles.searchClear} onClick={onClear} aria-label="Clear">
					<Icon name="cross-circle" />
				</button>
			)}
		</div>
	);
};

/**
 * A run of {@link PopupItem}s under an optional heading. Sections divide from one another, so a
 * popup can group its rows without the last group drawing a line against the container's edge.
 *
 * A plain div by default. Pass `render` to make the section a list primitive's own group, which is
 * how a combobox's groups label their own rows without restating the section's chrome.
 *
 * @public
 */
export const PopupSection: FC<{ label?: ReactNode } & useRender.ComponentProps<"div">> = ({
	label,
	children,
	render,
	...props
}) =>
	useRender({
		render: render ?? <div />,
		props: mergeProps<"div">(props, {
			className: styles.section,
			children: (
				<>
					{label !== undefined && (
						<div className={classes("text-12", styles.sectionLabel)}>{label}</div>
					)}
					<div className={styles.sectionItems}>{children}</div>
				</>
			),
		}),
	});

/**
 * One row of a popup: an optional glyph, the label, and — at the far end — a shortcut and a
 * trailing glyph marking what the row is or where it leads.
 *
 * A button by default. Pass `render` to make it whatever a list primitive needs it to be, which is
 * how a picker's rows become `Combobox.Item`s without restating the row's chrome.
 *
 * @public
 */
export const PopupItem: FC<
	{
		/** Leads the row — what kind of thing it is. */
		icon?: IconName;
		/** Closes the row — a tick for the current choice, a chevron for a step further in. */
		trailing?: IconName;
		/** The shortcut that does what the row does, sat before any trailing glyph. */
		kbd?: string | HotkeySequence;
		children: ReactNode;
	} & useRender.ComponentProps<"button">
> = ({ icon, trailing, kbd, children, render, ...props }) =>
	useRender({
		// oxlint-disable-next-line jsx_a11y/control-has-associated-label -- Labelled by its children.
		render: render ?? <button type="button" />,
		props: mergeProps<"button">(props, {
			className: classes("text-13", styles.item),
			children: (
				<>
					{icon !== undefined && <Icon name={icon} className={styles.itemIcon} />}
					<span className={styles.itemLabel}>{children}</span>
					{kbd !== undefined && <Kbd hotkey={kbd} className={styles.itemKbd} />}
					{trailing !== undefined && <Icon name={trailing} className={styles.itemIcon} />}
				</>
			),
		}),
	});
