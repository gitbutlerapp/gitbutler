import { classes } from "./classes.ts";
import type { IconName } from "./iconNames.ts";
import styles from "./Menu.module.css";
import { Popup, PopupItem, PopupSection } from "./Popup.tsx";
import { Tooltip } from "./Tooltip.tsx";
import { ContextMenu as BaseContextMenu, Menu as BaseMenu } from "@base-ui/react";
import type { HotkeySequence } from "@tanstack/react-hotkeys";
import type { ComponentProps, FC, ReactElement, ReactNode } from "react";

/** Where a menu without a trigger hangs: under an element, or at a point (where a click landed). */
export type MenuAnchor = Element | { x: number; y: number };

const toAnchor = (anchor: MenuAnchor | null | undefined) =>
	anchor == null || anchor instanceof Element
		? anchor
		: { getBoundingClientRect: () => DOMRect.fromRect({ x: anchor.x, y: anchor.y }) };

/** The popup every menu wears: the one overlay container, scrolling when the window is shorter than its list. */
const MenuPopup: FC<
	{
		popup: ReactElement;
		side?: "top" | "bottom" | "left" | "right";
		align?: "start" | "center" | "end";
		sideOffset?: number;
		anchor?: ReturnType<typeof toAnchor>;
	} & ComponentProps<"div">
> = ({ popup, side, align, sideOffset, anchor, className, children, ...props }) => (
	<BaseMenu.Portal>
		<BaseMenu.Positioner
			side={side}
			align={align}
			sideOffset={sideOffset}
			anchor={anchor ?? undefined}
		>
			<Popup anchored {...props} className={classes(className, styles.menu)} render={popup}>
				<div className={styles.list}>{children}</div>
			</Popup>
		</BaseMenu.Positioner>
	</BaseMenu.Portal>
);

/** @public */
export type MenuProps = {
	/** Opens the menu and anchors it. Leave it out to open the menu from code, at `anchor`. */
	trigger?: ReactElement;
	/** Where a menu without a trigger hangs: an element, or a point such as where a click landed. */
	anchor?: MenuAnchor | null;
	/** Omit to leave the menu uncontrolled and drive it from `trigger` alone. */
	open?: boolean;
	onOpenChange?: (open: boolean) => void;
	/** @default "bottom" */
	side?: "top" | "bottom" | "left" | "right";
	/** @default "start" */
	align?: "start" | "center" | "end";
	/** @default 4 */
	sideOffset?: number;
} & ComponentProps<"div">;

/**
 * A list of actions, opened by a button or from code: the ⋯ on a row, a header's options. Its rows
 * are {@link MenuItem}s, grouped into {@link MenuSection}s, and a choice of one is a
 * {@link MenuRadioGroup}. It wears the same container as every other overlay, anchored to what
 * opened it, and is never narrower than that.
 *
 * For a menu raised by a right-click on an area, use {@link ContextMenu}; the rows are the same.
 * For an anchored panel that is not a list of actions (a filter, a notification list), use
 * `Dropdown`.
 *
 * Without a `trigger`, control `open` and pass `anchor`: an element to hang under, or a point.
 * Pressing an element anchor while the menu is open is left to that element's own click, which
 * toggles the menu, so the press doesn't close it only for the click to open it again.
 *
 * @import import { Menu, MenuItem, MenuSection } from "@gitbutler/ui-react/Menu.tsx";
 */
export const Menu: FC<MenuProps> = ({
	trigger,
	anchor,
	open,
	onOpenChange,
	side = "bottom",
	align = "start",
	sideOffset = 4,
	...props
}) => (
	<BaseMenu.Root
		open={open}
		onOpenChange={(next, details) => {
			const pressed = details.event.target;
			if (!next && anchor instanceof Element && pressed instanceof Node && anchor.contains(pressed))
				return;
			onOpenChange?.(next);
		}}
	>
		{trigger !== undefined && <BaseMenu.Trigger render={trigger} />}
		<MenuPopup
			{...props}
			popup={<BaseMenu.Popup />}
			side={side}
			align={align}
			sideOffset={sideOffset}
			anchor={toAnchor(anchor)}
		/>
	</BaseMenu.Root>
);

/** @public */
export type ContextMenuProps = {
	/** The area a right-click (or a long press) opens the menu on. */
	trigger: ReactElement;
	onOpenChange?: (open: boolean) => void;
	/** Leaves the browser's own menu to the area, as while it is busy. */
	disabled?: boolean;
} & ComponentProps<"div">;

/**
 * The menu a right-click raises on an area, at the pointer: the same rows as {@link Menu}'s, for
 * the same actions the area's ⋯ offers.
 *
 * @import import { ContextMenu, MenuItem } from "@gitbutler/ui-react/Menu.tsx";
 */
export const ContextMenu: FC<ContextMenuProps> = ({
	trigger,
	onOpenChange,
	disabled,
	...props
}) => (
	<BaseContextMenu.Root
		disabled={disabled}
		onOpenChange={onOpenChange ? (next) => onOpenChange(next) : undefined}
	>
		<BaseContextMenu.Trigger render={trigger} />
		<MenuPopup {...props} popup={<BaseContextMenu.Popup />} />
	</BaseContextMenu.Root>
);

/**
 * A run of a menu's rows under an optional heading. Sections divide from one another; that line
 * is a menu's only separator.
 *
 * @import import { Menu, MenuItem, MenuSection } from "@gitbutler/ui-react/Menu.tsx";
 */
export const MenuSection: FC<{ label?: string; children: ReactNode }> = ({ label, children }) => (
	<PopupSection label={label} render={<BaseMenu.Group aria-label={label} />}>
		{children}
	</PopupSection>
);

type RowProps = {
	/** Leads the row: what kind of thing it acts on. */
	icon?: IconName;
	/** The shortcut that does the same, at the row's end. */
	kbd?: string | HotkeySequence;
	children: ReactNode;
};

/** A held row says why on hover: the menu already shows what it is, and the reason is a detail. */
const Held: FC<{ hint: ReactNode | undefined; children: ReactElement }> = ({ hint, children }) =>
	hint === undefined ? (
		children
	) : (
		<Tooltip side="right" content={hint}>
			{children}
		</Tooltip>
	);

/**
 * One action in a menu: its label, an optional leading glyph and its shortcut. Choosing it runs
 * `onClick` and closes the menu.
 *
 * A row that can't run now stays in place, `disabled`, so the menu keeps its shape; `hint` says
 * why on hover ("The machine is offline"). An action that destroys something reads as any other
 * row: the confirmation it opens is where the danger is said.
 *
 * @import import { Menu, MenuItem } from "@gitbutler/ui-react/Menu.tsx";
 */
export const MenuItem: FC<
	RowProps & {
		disabled?: boolean;
		/** Why the row is disabled, shown on hover. */
		hint?: ReactNode;
		onClick?: () => void;
	}
> = ({ icon, kbd, disabled = false, hint, onClick, children }) => (
	<Held hint={disabled ? hint : undefined}>
		<BaseMenu.Item
			disabled={disabled}
			onClick={onClick}
			render={(props) => (
				<PopupItem {...props} icon={icon} kbd={kbd}>
					{children}
				</PopupItem>
			)}
		/>
	</Held>
);

/**
 * A choice of one, as a section of a menu: the current {@link MenuRadioItem} carries a tick, and
 * picking another one sets it and closes the menu. "Group by", "Sort by", a theme.
 *
 * @import import { Menu, MenuRadioGroup, MenuRadioItem } from "@gitbutler/ui-react/Menu.tsx";
 */
export const MenuRadioGroup: FC<{
	label?: string;
	value: string;
	onValueChange: (value: string) => void;
	children: ReactNode;
}> = ({ label, value, onValueChange, children }) => (
	<PopupSection
		label={label}
		render={
			<BaseMenu.RadioGroup
				aria-label={label}
				value={value}
				onValueChange={(next: string) => onValueChange(next)}
			/>
		}
	>
		{children}
	</PopupSection>
);

/**
 * One option in a {@link MenuRadioGroup}, ticked while it is the group's value.
 *
 * @import import { Menu, MenuRadioGroup, MenuRadioItem } from "@gitbutler/ui-react/Menu.tsx";
 */
export const MenuRadioItem: FC<RowProps & { value: string; disabled?: boolean }> = ({
	value,
	disabled = false,
	icon,
	kbd,
	children,
}) => (
	<BaseMenu.RadioItem
		value={value}
		disabled={disabled}
		closeOnClick
		render={(props, state) => (
			<PopupItem {...props} icon={icon} kbd={kbd} trailing={state.checked ? "tick" : undefined}>
				{children}
			</PopupItem>
		)}
	/>
);
