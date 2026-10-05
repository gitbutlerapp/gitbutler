/** The harness panel's popup (`harness/deepseek/client.js`), typed; keep the two in step. */
import type { NativeMenuPopupItem, ShowNativeMenuParams } from "#electron/ipc.ts";
import {
	type CSSProperties,
	type FC,
	type ReactNode,
	useEffect,
	useLayoutEffect,
	useRef,
	useState,
} from "react";

export type OpenMenu = ShowNativeMenuParams & {
	resolve: (itemId: string | null) => void;
	close: () => void;
};

// The items carry Electron-style accelerators, which a page has no OS to convert, so they show as
// the glyphs macOS users expect instead of "CommandOrControl+X".
const ACCELERATOR_SYMBOLS: Record<string, string> = {
	CommandOrControl: "⌘",
	CmdOrCtrl: "⌘",
	Command: "⌘",
	Control: "⌃",
	Ctrl: "⌃",
	Shift: "⇧",
	Alt: "⌥",
	Option: "⌥",
	Enter: "↵",
	Return: "↵",
	Backspace: "⌫",
	Delete: "⌫",
	Escape: "⎋",
	Esc: "⎋",
	Tab: "⇥",
	Space: "␣",
	ArrowUp: "↑",
	ArrowDown: "↓",
	ArrowLeft: "←",
	ArrowRight: "→",
	PageUp: "⇞",
	PageDown: "⇟",
	Home: "↖",
	End: "↘",
};
const toSymbolAccelerator = (accelerator: string) =>
	accelerator
		.split("+")
		.map((part) => ACCELERATOR_SYMBOLS[part] ?? part)
		.join("");

const itemBase: CSSProperties = {
	display: "flex",
	alignItems: "center",
	gap: 10,
	padding: "5px 10px",
	borderRadius: 5,
	cursor: "pointer",
	position: "relative",
};
const hoverBg = "color-mix(in srgb, currentColor 14%, transparent)";
const submenuBase: CSSProperties = {
	minWidth: 190,
	background: "Canvas",
	color: "CanvasText",
	border: "1px solid color-mix(in srgb, currentColor 30%, transparent)",
	borderRadius: 8,
	boxShadow: "0 8px 24px rgba(0,0,0,.3)",
	padding: 4,
};
const MARGIN = 4;

export const NativeMenu: FC<{ menu: OpenMenu }> = ({ menu }) => {
	const rootRef = useRef<HTMLDivElement>(null);
	const [openPath, setOpenPath] = useState<Array<number>>([]);
	const closeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
	const itemRefs = useRef(new Map<string, HTMLDivElement>());
	const submenuRef = useRef<HTMLDivElement | null>(null);

	const cancelTimer = () => {
		if (closeTimerRef.current !== null) clearTimeout(closeTimerRef.current);
		closeTimerRef.current = null;
	};
	const armTimer = (fn: () => void) => {
		cancelTimer();
		closeTimerRef.current = setTimeout(() => {
			closeTimerRef.current = null;
			fn();
		}, 120);
	};

	useEffect(() => cancelTimer, []);
	// Dismissal from anywhere outside, as a native menu does.
	useEffect(() => {
		const onKey = (event: KeyboardEvent) => {
			if (event.key === "Escape") menu.close();
		};
		const onPointer = (event: PointerEvent) => {
			if (rootRef.current && !rootRef.current.contains(event.target as Node)) menu.close();
		};
		document.addEventListener("keydown", onKey, true);
		document.addEventListener("pointerdown", onPointer, true);
		window.addEventListener("blur", menu.close);
		return () => {
			document.removeEventListener("keydown", onKey, true);
			document.removeEventListener("pointerdown", onPointer, true);
			window.removeEventListener("blur", menu.close);
		};
	}, [menu]);

	// Clamp the root menu into the viewport once it has rendered.
	useLayoutEffect(() => {
		const root = rootRef.current;
		if (!root) return;
		const rect = root.getBoundingClientRect();
		root.style.left = `${Math.max(MARGIN, Math.min(rect.left, window.innerWidth - rect.width - MARGIN))}px`;
		root.style.top = `${Math.max(MARGIN, Math.min(rect.top, window.innerHeight - rect.height - MARGIN))}px`;
	}, [menu]);

	// Clamp the deepest open submenu into the viewport: flip it left when it would cross the right
	// edge, and keep it off the bottom.
	useLayoutEffect(() => {
		const submenu = submenuRef.current;
		const itemEl = itemRefs.current.get(openPath.join("/"));
		if (!submenu || !itemEl) return;
		const ir = itemEl.getBoundingClientRect();
		const sr = submenu.getBoundingClientRect();
		const flip = ir.right + sr.width > window.innerWidth - MARGIN && ir.left - sr.width >= MARGIN;
		const left = Math.max(
			MARGIN,
			Math.min(flip ? ir.left - sr.width : ir.right, window.innerWidth - sr.width - MARGIN),
		);
		const top = Math.max(MARGIN, Math.min(ir.top - 5, window.innerHeight - sr.height - MARGIN));
		Object.assign(submenu.style, { position: "fixed", left: `${left}px`, top: `${top}px` });
	}, [openPath]);

	const renderItems = (items: Array<NativeMenuPopupItem>, depth: number): Array<ReactNode> =>
		items.map((item, index) => {
			if (item._tag === "Separator") {
				return (
					<div
						// oxlint-disable-next-line react/no-array-index-key -- Separators have nothing else to tell them apart.
						key={`${depth}-${index}`}
						style={{
							height: 1,
							background: "color-mix(in srgb, currentColor 20%, transparent)",
							margin: "4px 6px",
						}}
					/>
				);
			}
			const path = [...openPath.slice(0, depth), index];
			const key = path.join("/");
			const isOpen = depth < openPath.length && openPath[depth] === index;
			const enabled = item.enabled !== false;
			const submenu = item.submenu !== undefined && item.submenu.length > 0 ? item.submenu : null;
			const isDeepest = depth === openPath.length - 1;
			const choose = () => {
				if (!enabled || item.itemId === undefined) return;
				menu.resolve(item.itemId);
				menu.close();
			};
			return (
				<div
					key={key}
					role="menuitem"
					aria-disabled={!enabled}
					ref={(el) => {
						if (el) itemRefs.current.set(key, el);
						else itemRefs.current.delete(key);
					}}
					style={{
						...itemBase,
						opacity: enabled ? 1 : 0.4,
						background: isOpen ? hoverBg : undefined,
					}}
					onMouseEnter={(event) => {
						event.currentTarget.style.background = enabled ? hoverBg : "";
						if (!enabled) return;
						armTimer(() => {
							setOpenPath((open) => (submenu !== null ? path : open.slice(0, depth)));
						});
					}}
					onMouseLeave={(event) => {
						if (!isOpen) event.currentTarget.style.background = "";
					}}
					tabIndex={-1}
					onClick={choose}
					onKeyDown={(event) => {
						if (event.key === "Enter") choose();
					}}
				>
					<span style={{ flex: 1 }}>{item.label}</span>
					{item.checked === true && <span>✓</span>}
					{item.accelerator !== undefined && (
						<span
							style={{
								color: "color-mix(in srgb, currentColor 60%, transparent)",
								fontSize: 11,
								letterSpacing: "0.04em",
							}}
						>
							{toSymbolAccelerator(item.accelerator)}
						</span>
					)}
					{submenu !== null && <span style={{ fontSize: 10 }}>▸</span>}
					{isOpen && submenu !== null && (
						<div
							role="menu"
							tabIndex={-1}
							ref={
								isDeepest
									? (el) => {
											submenuRef.current = el;
										}
									: undefined
							}
							style={{ ...submenuBase, position: "absolute", left: "100%", top: -5 }}
							onMouseEnter={cancelTimer}
						>
							{renderItems(submenu, depth + 1)}
						</div>
					)}
				</div>
			);
		});

	return (
		<div
			ref={rootRef}
			role="menu"
			tabIndex={-1}
			style={{
				position: "fixed",
				left: menu.position.x,
				top: menu.position.y,
				zIndex: 2147483000,
				minWidth: 200,
				background: "Canvas",
				color: "CanvasText",
				border: "1px solid color-mix(in srgb, currentColor 30%, transparent)",
				borderRadius: 8,
				boxShadow: "0 8px 24px rgba(0,0,0,.3)",
				padding: 4,
				fontFamily: "system-ui, sans-serif",
				fontSize: 13,
			}}
		>
			{renderItems(menu.items, 0)}
		</div>
	);
};
