import { Button } from "./Button.tsx";
import { classes } from "./classes.ts";
import { EmptyState } from "./EmptyState.tsx";
import { Icon } from "./Icon.tsx";
import type { IconName } from "./iconNames.ts";
import { Kbd } from "./Kbd.tsx";
import styles from "./Popup.module.css";
import { ScrollArea } from "./ScrollArea.tsx";
import { AlertDialog, Dialog, mergeProps, Popover, useRender } from "@base-ui/react";
import type { HotkeySequence } from "@tanstack/react-hotkeys";
import {
	useEffect,
	useId,
	useRef,
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
 * @import import { Popup } from "@gitbutler/ui-react/Popup.tsx";
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
 * How wide a modal opens, picked by what it holds rather than measured per caller. Each is a
 * design-core token, `--size-modal-xs` to `--size-modal-lg`:
 *
 * - `xsmall` (320): a one-line question and its buttons — "Discard 3 files?".
 * - `small` (420): a prompt with a field or a short list, and pickers.
 * - `medium` (520): a pane that lays itself out around one group.
 * - `large` (820): a pane with more than one group, or a working surface: settings, the conflict
 *   resolver.
 *
 * @public
 */
export type ModalSize = "xsmall" | "small" | "medium" | "large";

const sizeClassName = (size: ModalSize): string | undefined =>
	({
		xsmall: styles.sizeXsmall,
		small: styles.sizeSmall,
		medium: styles.sizeMedium,
		large: styles.sizeLarge,
	})[size];

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
	/** @default "small" */
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
 * The modal owns its chrome and placement. A prompt, a confirmation or a short form is built from
 * {@link ModalHeader}, {@link ModalBody} and {@link ModalFooter}, and a pane as involved as settings
 * lays itself out entirely. A list to search and pick from is not a modal's: it is a `PickerDialog`,
 * or Popup's combobox when it hangs off the control that opened it.
 *
 * @public
 * @import import { Modal } from "@gitbutler/ui-react/Popup.tsx";
 */
export const Modal: FC<ModalProps> = ({
	open,
	onOpenChange,
	trigger,
	alert = false,
	size = "small",
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

/**
 * A modal's title, and the one line under it that says what answering does. The first of the three
 * parts a prompt, a confirmation or a short form is built from, above {@link ModalBody} and
 * {@link ModalFooter}. The title is the dialog's accessible name and the description its
 * description, so neither needs restating in an `aria-label`.
 *
 * Pass `onBack` on a step the modal drilled into — a choice picked from a list in the same modal —
 * and the header leads with a Back button that returns to it. Back is for that one level in, not
 * for walking a sequence of steps. The pick that had focus is gone once the step opens, so Back
 * takes focus; on returning, the caller focuses the pick it came from.
 *
 * There is no close button: Cancel, Escape and the backdrop dismiss a modal, and an `alert` one
 * must be answered.
 *
 * @public
 * @import import { Modal, ModalBody, ModalFooter, ModalHeader } from "@gitbutler/ui-react/Popup.tsx";
 */
export const ModalHeader: FC<
	{
		title: ReactNode;
		/** What happens on answering, or what the user needs to know first. One or two sentences. */
		description?: ReactNode;
		/** Returns to the step this one was opened from. Draws a Back button before the title. */
		onBack?: () => void;
	} & ComponentProps<"div">
> = ({ title, description, onBack, className, ...props }) => {
	const titleElement = (
		<Dialog.Title className={classes("text-14", "text-semibold", styles.modalTitle)}>
			{title}
		</Dialog.Title>
	);

	return (
		<div {...props} className={classes(className, styles.modalPart, styles.modalHeader)}>
			{onBack === undefined ? (
				titleElement
			) : (
				<div className={styles.modalTitleRow}>
					<Button
						variant="ghost"
						size="small"
						iconOnly
						aria-label="Back"
						onClick={onBack}
						// oxlint-disable-next-line jsx_a11y/no-autofocus -- The pick that opened this step unmounted with focus on it.
						autoFocus
					>
						<Icon name="arrow-left" />
					</Button>
					{titleElement}
				</div>
			)}
			{description !== undefined && (
				<Dialog.Description className={classes("text-13", "text-body", styles.modalDescription)}>
					{description}
				</Dialog.Description>
			)}
		</div>
	);
};

/**
 * What a modal asks about: fields, a list of the things it acts on, a note. Stacks its children
 * apart and scrolls when the modal outgrows the window, while {@link ModalHeader} and
 * {@link ModalFooter} stay in place. While it scrolls, a hairline marks each edge it runs past:
 * under the header once it's scrolled down, over the footer while there is more below.
 *
 * @public
 * @import import { Modal, ModalBody, ModalFooter, ModalHeader } from "@gitbutler/ui-react/Popup.tsx";
 */
export const ModalBody: FC<ComponentProps<"div">> = ({ className, ...props }) => (
	<ScrollArea
		className={styles.modalBodyArea}
		viewportClassName={classes(className, styles.modalBody)}
		{...props}
	/>
);

/**
 * A modal's buttons, right-aligned with the one that answers last: Cancel as `ghost`, then the
 * answer as `gray`, or `danger` when it destroys something. Not `pop`: a modal with two buttons is
 * not the busy surface pop is kept for (see Emphasis in the design notes).
 *
 * A form that submits on Enter wraps all three parts, header to footer, with the submit button
 * here; the modal lays a form out as it would the parts themselves.
 *
 * @public
 * @import import { Modal, ModalBody, ModalFooter, ModalHeader } from "@gitbutler/ui-react/Popup.tsx";
 */
export const ModalFooter: FC<ComponentProps<"div">> = ({ className, ...props }) => (
	<div {...props} className={classes(className, styles.modalPart, styles.modalFooter)} />
);

/** @public */
export type DropdownProps = {
	/** Omit to leave the dropdown uncontrolled and drive it from `trigger` alone. */
	open?: boolean;
	onOpenChange?: (open: boolean) => void;
	/** Rendered as the dropdown's trigger, and what it anchors to. */
	trigger: ReactElement;
	/**
	 * Also opens while the pointer rests on the trigger, and stays while it moves into the panel:
	 * for a panel that shows at a glance what the trigger keeps hidden. Opened by hover, it leaves
	 * focus where it was. While a field in it has focus and something typed, as a search in use,
	 * the pointer leaving no longer shuts it, and Escape or a click outside does; a click on a row
	 * doesn't hold it, so it still goes when the pointer leaves. The pointer moving onto a tooltip or menu opened
	 * from inside it doesn't shut it either: those float outside the panel but belong to it, and it
	 * goes once they do with the pointer still outside.
	 */
	openOnHover?: boolean;
	/**
	 * How long the pointer rests on the trigger before an `openOnHover` dropdown opens, in ms:
	 * long enough to pass over it on the way elsewhere, short enough to read as a glance.
	 * @default 100
	 */
	delay?: number;
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
 * This is for anchored *panels* — a notification list, a reaction picker, a filter. A list of
 * actions is a `Menu`, from `Menu.tsx`; a dropdown is not the place to rebuild one.
 *
 * It is as wide as its content, and never narrower than its trigger: a short list under a wide
 * control lines up with it, and a long one grows past it.
 *
 * @public
 * @import import { Dropdown } from "@gitbutler/ui-react/Popup.tsx";
 */
export const Dropdown: FC<DropdownProps> = ({
	open,
	onOpenChange,
	trigger,
	openOnHover = false,
	delay = 100,
	side = "bottom",
	align = "start",
	sideOffset = 4,
	children,
	className,
	...props
}) => {
	const id = useId();
	const actions = useRef<Popover.Root.Actions>(null);
	const stopWatching = useRef<() => void>(undefined);
	useEffect(() => () => stopWatching.current?.(), []);
	const panel = `[data-dropdown="${id}"]`;
	// Only a field with something in it: an empty one has nothing to lose, and clearing a search
	// leaves the caret in its field, which would otherwise hold the panel open for good.
	const typing = () => {
		const field = document.querySelector(`${panel} :is(input, textarea, [contenteditable]):focus`);
		if (field instanceof HTMLInputElement || field instanceof HTMLTextAreaElement)
			return field.value !== "";
		return Boolean(field?.textContent);
	};
	// A tooltip or menu a control in it opened floats outside the panel; the control stays marked
	// open while it shows. Only a control that opens a popup: an expanded folder in a file tree is
	// `aria-expanded` too, and would hold the panel open for as long as the folder is.
	const layered = () =>
		document.querySelector(
			`${panel} :is([data-popup-open], [aria-haspopup][aria-expanded="true"])`,
		);
	return (
		<Popover.Root
			actionsRef={actions}
			open={open}
			onOpenChange={(next, details) => {
				stopWatching.current?.();
				if (!next && details.reason === "trigger-hover") {
					// The reader is typing in it: the pointer wandering off is no reason to take that away.
					if (typing()) return details.cancel();
					// The pointer went onto something layered over it. Back in the panel, the hover
					// carries on as before; once that layer is gone with the pointer still out, it's over.
					if (layered()) {
						details.cancel();
						const stop = () => {
							document.removeEventListener("pointermove", watch);
							stopWatching.current = undefined;
						};
						const watch = (event: PointerEvent) => {
							if (event.target instanceof Element && event.target.closest(panel)) return stop();
							if (layered()) return;
							stop();
							actions.current?.close();
						};
						document.addEventListener("pointermove", watch);
						stopWatching.current = stop;
						return;
					}
				}
				onOpenChange?.(next);
			}}
		>
			<Popover.Trigger render={trigger} openOnHover={openOnHover} delay={delay} />
			<Popover.Portal>
				<Popover.Positioner side={side} align={align} sideOffset={sideOffset}>
					<Popup
						anchored
						{...props}
						data-dropdown={id}
						className={classes(className, styles.dropdownPanel)}
						render={<Popover.Popup />}
					>
						{children}
					</Popup>
				</Popover.Positioner>
			</Popover.Portal>
		</Popover.Root>
	);
};

/**
 * Puts the caret back in the search of the popup `from` sits in. A clear by pointer otherwise
 * leaves focus on a button about to unmount, and so nowhere, when the field is what the next
 * keystroke is meant to reach. Found in the DOM rather than through a ref so the callers, which
 * hold the query but not always the input, have nothing to wire.
 */
const focusSearch = (from: Element) => {
	const { popup, searchInput } = styles;
	if (popup === undefined || searchInput === undefined) return;
	from.closest(`.${popup}`)?.querySelector<HTMLInputElement>(`.${searchInput}`)?.focus();
};

/**
 * The filter row a popup leads with. Full-bleed against the container's top edge, divided from the
 * list below by a hairline rather than fenced off in a field of its own — the popup is already the
 * box the query sits in.
 *
 * A plain input by default. Pass `render` when the query drives a list primitive and the input has
 * to be that primitive's own — a picker's `Autocomplete.Input`, say.
 *
 * Pass `onClear` while there is a query to clear: the row's trailing glyph becomes a button that
 * empties the field and puts the caret back in it. The row does not hold the query, so whether
 * there is anything to clear is the caller's to say — leave `onClear` out and the row shows the
 * magnifier it searches under.
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
				<button
					type="button"
					className={styles.searchClear}
					onClick={(event) => {
						onClear();
						focusSearch(event.currentTarget);
					}}
					aria-label="Clear"
				>
					<Icon name="cross-circle" />
				</button>
			)}
		</div>
	);
};

/**
 * What a popup's list shows when it has no rows: the "Empty state" block in its `tidy` variant,
 * sized for a list rather than a panel.
 *
 * A search that came up empty reports the miss in `nothingFound` with a small "Clear search" under
 * it, which calls `onClearSearch` — the same clear the search row's own button does, so pass that —
 * and puts the caret back in the popup's search.
 * No drawing: the line sits where the rows were, and the way back is right under it. A list with
 * nothing in it before anything was typed gets the cactus and `nothingToList` instead, since
 * nothing was searched for and "found" would be the wrong word. Pass the `query` the list is
 * filtered on — the deferred one, where the caller defers — so the block and the rows it stands in
 * for agree.
 *
 * Both lines are one short line, no full stop, like every other line of their length in Lite:
 * "No hotkeys found", "Nothing to restore yet".
 *
 * For a popup as wide as a picker. A dropdown no wider than its trigger — the commit target
 * combobox — keeps a plain line.
 *
 * @public
 */
export const PopupEmpty: FC<{
	query: string;
	nothingFound: string;
	nothingToList: string;
	onClearSearch: () => void;
}> = ({ query, nothingFound, nothingToList, onClearSearch }) =>
	query === "" ? (
		<EmptyState
			variant="tidy"
			illustration="cactus"
			description={nothingToList}
			className={styles.empty}
		/>
	) : (
		<EmptyState variant="tidy" description={nothingFound} className={styles.empty}>
			<Button
				variant="outline"
				size="small"
				onClick={(event) => {
					onClearSearch();
					focusSearch(event.currentTarget);
				}}
			>
				Clear search
			</Button>
		</EmptyState>
	);

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
 * A section's heading on its own, for a list whose rows a virtualiser positions one by one and so
 * cannot wrap in a {@link PopupSection}. The same heading a section draws; `divided` adds the
 * hairline the section before it would otherwise have drawn.
 *
 * @public
 */
export const PopupSectionLabel: FC<{ divided?: boolean } & useRender.ComponentProps<"div">> = ({
	divided = false,
	render,
	...props
}) =>
	useRender({
		render: render ?? <div />,
		props: mergeProps<"div">(props, {
			className: classes("text-12", styles.sectionLabel, divided && styles.sectionLabelDivided),
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
		/**
		 * Leads the row where a glyph from the icon set is not the right mark — a program's own
		 * image, say. Sits after `icon` when both are given.
		 */
		leading?: ReactNode;
		/** Closes the row — a tick for the current choice, a chevron for a step further in. */
		trailing?: IconName;
		/** The shortcut that does what the row does, sat before any trailing glyph. */
		kbd?: string | HotkeySequence;
		children: ReactNode;
	} & useRender.ComponentProps<"button">
> = ({ icon, leading, trailing, kbd, children, render, ...props }) =>
	useRender({
		// oxlint-disable-next-line jsx_a11y/control-has-associated-label -- Labelled by its children.
		render: render ?? <button type="button" />,
		props: mergeProps<"button">(props, {
			className: classes("text-13", styles.item),
			children: (
				<>
					{icon !== undefined && <Icon name={icon} className={styles.itemIcon} />}
					{leading}
					<span className={styles.itemLabel}>{children}</span>
					{kbd !== undefined && <Kbd hotkey={kbd} className={styles.itemKbd} />}
					{trailing !== undefined && <Icon name={trailing} className={styles.itemIcon} />}
				</>
			),
		}),
	});
