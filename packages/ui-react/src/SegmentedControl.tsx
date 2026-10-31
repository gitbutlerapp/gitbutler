import { Toggle, ToggleGroup } from "@base-ui/react";
import type { HotkeySequence } from "@tanstack/react-hotkeys";
import { createContext, useContext, useState, type FC, type ReactNode, type Ref } from "react";
import { Badge } from "./Badge.tsx";
import { classes } from "./classes.ts";
import { Kbd } from "./Kbd.tsx";
import styles from "./SegmentedControl.module.css";

/** @public */
export type SegmentedControlSize = "regular" | "small";

const SizeContext = createContext<SegmentedControlSize>("regular");

// Puts the track's raised ground under the picked segment, and keeps it there as the pick moves
// or the segments change size. CSS anchors could do this, but Chromium stops moving a transitioned
// anchored box once its anchor changes size, so a font load or a new count leaves it behind.
const placeGround = (track: HTMLElement | null) => {
	if (!track) return;
	const place = () => {
		const picked = track.querySelector<HTMLElement>("[data-pressed]");
		track.toggleAttribute("data-picked", picked !== null);
		if (picked === null) return;
		// Boxes rather than offsets, which round to whole pixels and leave the ground off its segment.
		const segment = picked.getBoundingClientRect();
		track.style.setProperty(
			"--picked-left",
			`${segment.left - track.getBoundingClientRect().left}px`,
		);
		track.style.setProperty("--picked-width", `${segment.width}px`);
	};
	const sizes = new ResizeObserver(place);
	const observeSegments = () => {
		for (const segment of track.children) sizes.observe(segment, { box: "border-box" });
	};
	const picks = new MutationObserver(() => {
		observeSegments();
		place();
	});
	place();
	observeSegments();
	picks.observe(track, { subtree: true, childList: true, attributeFilter: ["data-pressed"] });
	const frame = requestAnimationFrame(() => track.toggleAttribute("data-sliding", true));
	return () => {
		cancelAnimationFrame(frame);
		sizes.disconnect();
		picks.disconnect();
	};
};

/**
 * Picks one value of a setting, a filter or a mode from a few {@link Segment}s, and always has one
 * picked: a diff's Split or Unified, a theme, whose notifications to show. The picked segment is
 * raised on a gray track, and the raised ground slides to the next one picked. The others sit
 * quieter, with a short divider between two of them; under the pointer one comes up on a ground.
 *
 * - Not for switching what a pane shows: that is `Tabs`. Not for switches that turn on and off on
 *   their own: that is a `ToggleGroup` with `multiple`.
 * - Controlled with `value` and `onValueChange`, or left to itself with `defaultValue`. Pressing
 *   the picked segment again keeps it picked.
 * - The arrow keys move between the segments.
 * - `size="small"` is 24px tall, for a header or a popup; it leaves a `count` room around it.
 * - Name it with `aria-label`, or `aria-labelledby` when a visible label already says what it picks.
 * - Other props go to the group, as they would on Base UI's `ToggleGroup`.
 *
 * @public
 * @import import { SegmentedControl, Segment } from "@gitbutler/ui-react/SegmentedControl.tsx";
 */
export const SegmentedControl = <Value extends string>({
	value,
	defaultValue,
	onValueChange,
	size = "regular",
	className,
	ref,
	children,
	...props
}: Omit<
	ToggleGroup.Props<Value>,
	"value" | "defaultValue" | "onValueChange" | "multiple" | "className" | "render"
> & {
	value?: Value;
	defaultValue?: Value;
	onValueChange?: (value: Value) => void;
	size?: SegmentedControlSize;
	className?: string;
	ref?: Ref<HTMLDivElement>;
	children: ReactNode;
}) => {
	const [uncontrolledValue, setUncontrolledValue] = useState(defaultValue);
	const picked = value ?? uncontrolledValue;
	// The track is the component's to measure and the caller's to hold, as a tooltip's trigger does.
	const track = (element: HTMLDivElement | null) => {
		const unplace = placeGround(element);
		if (typeof ref === "function") ref(element);
		else if (ref) ref.current = element;
		return () => {
			unplace?.();
			if (typeof ref === "function") ref(null);
			else if (ref) ref.current = null;
		};
	};
	return (
		<SizeContext value={size}>
			<ToggleGroup
				{...props}
				ref={track}
				value={picked === undefined ? [] : [picked]}
				// Base UI empties the group when the pressed toggle is pressed again; one stays picked.
				onValueChange={([next]: Array<Value>) => {
					if (next === undefined) return;
					setUncontrolledValue(next);
					onValueChange?.(next);
				}}
				className={classes(className, styles.control, size === "small" && styles.small)}
			>
				{children}
			</ToggleGroup>
		</SizeContext>
	);
};

/**
 * One option in a {@link SegmentedControl}: its label, an optional `icon` before it and an
 * optional `count` after it, for how many things the option holds. `kbd` names the shortcut that
 * picks it, faint after the label.
 *
 * A `disabled` segment is unavailable for now and stays at half strength; say why nearby, as a
 * tooltip or a hint.
 *
 * @public
 */
export const Segment: FC<
	Omit<Toggle.Props, "className" | "value" | "pressed" | "defaultPressed" | "onPressedChange"> & {
		value: string;
		className?: string;
		icon?: ReactNode;
		count?: number;
		kbd?: string | HotkeySequence;
	}
> = ({ className, icon, count, kbd, children, ...props }) => {
	const size = useContext(SizeContext);
	return (
		<Toggle
			{...props}
			className={classes(
				className,
				"text-semibold",
				size === "small" ? "text-12" : "text-13",
				styles.segment,
			)}
		>
			{icon !== undefined && <span className={styles.icon}>{icon}</span>}
			<span className={styles.label}>
				<span className={styles.text}>{children}</span>
				{kbd !== undefined && <Kbd hotkey={kbd} variant="button" />}
			</span>
			{count !== undefined && <Badge variant="lightGray">{count}</Badge>}
		</Toggle>
	);
};
