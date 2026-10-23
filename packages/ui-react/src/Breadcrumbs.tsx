import { Menu as BaseMenu } from "@base-ui/react";
import {
	useLayoutEffect,
	useRef,
	useState,
	type ComponentProps,
	type FC,
	type MouseEvent,
	type ReactNode,
} from "react";
import styles from "./Breadcrumbs.module.css";
import { classes } from "./classes.ts";
import { Icon } from "./Icon.tsx";
import { Menu, MenuSection } from "./Menu.tsx";
import { PopupItem } from "./Popup.tsx";
import { Tooltip } from "./Tooltip.tsx";

/** @public */
export type Crumb = {
	label: string;
	/** Where the level opens. A level with neither this nor `onClick` is only a name. */
	href?: string;
	/** For a host that routes in code: call `preventDefault` on the event to keep the link's own navigation from running. */
	onClick?: (event: MouseEvent<HTMLElement>) => void;
	/**
	 * The level and its siblings to jump between, as a {@link Menu}'s rows: a `MenuRadioGroup`
	 * of the machines, repos or worktrees, the current one ticked. Given, the level ends in a ⌃⌄
	 * that opens them.
	 */
	menu?: ReactNode;
};

type Props = {
	/** The levels above the page, outermost first: the page itself is never one of them. */
	items: Array<Crumb>;
} & Omit<ComponentProps<"nav">, "children">;

const linked = (crumb: Crumb) => crumb.href !== undefined || crumb.onClick !== undefined;

/**
 * The ⌃⌄ that ends a level with a menu: 18×22, its glyph quiet until the pointer is on it. A
 * level's `menu` draws it; on its own it is the trigger for a host's popup beside words that read
 * like a level, as the location bar's "Also in 2 places". Give it an `aria-label`.
 * @import import { CrumbMenuButton } from "@gitbutler/ui-react/Breadcrumbs.tsx";
 */
export const CrumbMenuButton: FC<Omit<ComponentProps<"button">, "children">> = ({
	className,
	...props
}) => (
	<button type="button" {...props} className={classes(className, styles.menu)}>
		<Icon name="chevron-up-down" size={10} />
	</button>
);

const Level: FC<{ crumb: Crumb }> = ({ crumb }) => (
	<>
		<Tooltip
			content={crumb.label}
			// The full name says something only when the level is cut short: whole, it would repeat it.
			onOpenChange={(open, { trigger, cancel }) => {
				if (open && trigger !== undefined && trigger.scrollWidth <= trigger.clientWidth) cancel();
			}}
		>
			{linked(crumb) ? (
				<a className={styles.crumb} href={crumb.href} onClick={crumb.onClick}>
					{crumb.label}
				</a>
			) : (
				<span className={styles.crumb}>{crumb.label}</span>
			)}
		</Tooltip>
		{crumb.menu !== undefined && (
			<Menu trigger={<CrumbMenuButton aria-label={`Switch from ${crumb.label}`} />}>
				{crumb.menu}
			</Menu>
		)}
	</>
);

const Separator: FC = () => <Icon name="chevron-right" size={10} className={styles.separator} />;

/**
 * Where a page sits: the levels above it, outermost first, a › between each. The page itself is
 * not one: the header under the trail names it. It keeps to one line: when the levels run out of
 * room, the ones between the first and the nearest fold into "…", the outermost
 * first, and open from there as a menu. A level still too long for what is left ends in an
 * ellipsis, its full name a hover away.
 *
 * A level with an `href` or an `onClick` is a link. A level with a `menu` ends in a ⌃⌄ that opens
 * its siblings, to jump across rather than up.
 *
 * @import import { Breadcrumbs } from "@gitbutler/ui-react/Breadcrumbs.tsx";
 */
export const Breadcrumbs: FC<Props> = ({ items, className, ...props }) => {
	const list = useRef<HTMLOListElement>(null);
	const measure = useRef<HTMLSpanElement>(null);
	const [folded, setFolded] = useState(0);

	// The levels' widths only exist once laid out, and the room changes with the pane: measure the
	// hidden copy before paint, and again whenever either resizes, so the trail never shows
	// overflowing for a frame.
	useLayoutEffect(() => {
		const nav = list.current?.parentElement;
		const el = list.current;
		const copy = measure.current;
		if (!nav || !el || !copy) return;
		// Unrounded, so the whole trail's width doesn't come out a pixel short of what it draws.
		const widthOf = (e: Element | null) => e?.getBoundingClientRect().width ?? 0;
		const fit = () => {
			const widths = [...copy.querySelectorAll("[data-level]")].map(widthOf);
			const separator = widthOf(copy.querySelector("[data-separator]"));
			const more = widthOf(copy.querySelector("[data-more]"));
			const columnGap = Number.parseFloat(getComputedStyle(el).columnGap);
			const gap = Number.isNaN(columnGap) ? 0 : columnGap;
			const width = (k: number) => {
				const shown = [widths[0] ?? 0, ...(k > 0 ? [more] : []), ...widths.slice(1 + k)];
				const joins = shown.length - 1;
				return shown.reduce((a, w) => a + w, 0) + joins * (separator + 2 * gap);
			};
			// The trail asks for its whole width, folded or not. Sized by its content instead, as in a
			// flex row, its room would be the folded trail's own width, and a trail that folded once
			// could never measure enough room to open again.
			nav.style.setProperty("--breadcrumbs-width", `${Math.ceil(width(0))}px`);
			let k = 0;
			while (k < items.length - 2 && width(k) > el.clientWidth) k++;
			setFolded(k);
		};
		fit();
		// The copy too: its levels widen when the web font arrives, while the list's own room stays put.
		const observer = new ResizeObserver(fit);
		observer.observe(el);
		observer.observe(copy);
		return () => observer.disconnect();
	}, [items]);

	const hidden = items.slice(1, 1 + folded);
	const shown = folded > 0 ? [items[0], null, ...items.slice(1 + folded)] : items;

	return (
		<nav
			aria-label="Breadcrumb"
			{...props}
			className={classes(className, "text-13", styles.breadcrumbs)}
		>
			<ol ref={list} className={styles.list}>
				{shown.map((crumb, i) => (
					<li key={crumb ? `${i}:${crumb.label}` : "more"} className={styles.level}>
						{i > 0 && <Separator />}
						{crumb ? (
							<Level crumb={crumb} />
						) : (
							<Menu
								trigger={
									<button
										type="button"
										className={styles.more}
										aria-label={`${hidden.length} more level${hidden.length === 1 ? "" : "s"}`}
									>
										…
									</button>
								}
							>
								<MenuSection>
									{hidden.map((h) =>
										h.href === undefined ? (
											<BaseMenu.Item
												key={h.label}
												onClick={h.onClick}
												nativeButton
												render={(itemProps) => <PopupItem {...itemProps}>{h.label}</PopupItem>}
											/>
										) : (
											<BaseMenu.LinkItem
												key={h.label}
												href={h.href}
												onClick={h.onClick}
												closeOnClick
												render={(itemProps) => (
													// The item's props are a link's: they go on the <a> the row renders as, not on
													// PopupItem, which is typed as a button.
													// oxlint-disable-next-line jsx_a11y/anchor-has-content -- The row renders as this link, and PopupItem fills it with the label.
													<PopupItem render={<a {...itemProps} href={h.href} />}>
														{h.label}
													</PopupItem>
												)}
											/>
										),
									)}
								</MenuSection>
							</Menu>
						)}
					</li>
				))}
			</ol>
			<span ref={measure} className={styles.measure} aria-hidden inert>
				{items.map((crumb) => (
					<span key={crumb.label} data-level className={styles.level}>
						{crumb.label}
						{crumb.menu !== undefined && <span className={styles.menu} />}
					</span>
				))}
				<span data-separator>
					<Separator />
				</span>
				<span data-more className={styles.more}>
					…
				</span>
			</span>
		</nav>
	);
};
