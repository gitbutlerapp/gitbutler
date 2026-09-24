import { classes } from "./classes.ts";
import styles from "./Range.module.css";
import { Field, Slider } from "@base-ui/react";
import type { CSSProperties, ReactNode } from "react";

/**
 * A stop on the scale under the track: a bare value reads as a number in the range's `format`,
 * and a value with a `label` reads as that label instead — "Off" for a zero, "1h" for a stop
 * whose number alone wouldn't say what it means.
 *
 * @public
 */
export type RangeMark = number | { value: number; label: ReactNode };

/** @public */
export type RangeProps<Value extends number | ReadonlyArray<number> = number> = {
	/**
	 * The field's label, above the control, with the current value reading at the other end of
	 * the same line. Omit for a range the surface labels some other way, and give it an
	 * `aria-label` instead; the value then goes unshown.
	 */
	label?: ReactNode;
	/**
	 * One number, or a pair for a span with a thumb at each end. Omit to leave the range
	 * uncontrolled.
	 */
	value?: Value;
	defaultValue?: Value;
	/** Fires on every change: each step of a drag, each arrow key. */
	onValueChange?: (value: Value extends number ? number : Value) => void;
	/**
	 * Fires once the user is done: on release of a drag, on a press of the track, with each arrow
	 * key. For a setting that should be written then rather than on every step.
	 */
	onValueCommitted?: (value: Value extends number ? number : Value) => void;
	/** @default 0 */
	min?: number;
	/** @default 100 */
	max?: number;
	/** What one arrow key adds, and where the thumb snaps to under a drag. Shift steps by `largeStep`. */
	step?: number;
	largeStep?: number;
	/**
	 * Values to write under the track as a scale, each centred on where the thumb sits at that
	 * value. For a range whose stops the user should be able to read off without dragging to
	 * them. A mark with a `label` shows that in place of its number.
	 */
	marks?: ReadonlyArray<RangeMark>;
	/** How the value reads in the header and on the scale — a unit, a fraction. */
	format?: Intl.NumberFormatOptions;
	disabled?: boolean;
	/** Identifies the field when a form is submitted. */
	name?: string;
	className?: string;
	style?: CSSProperties;
	"aria-label"?: string;
};

/**
 * A number the user finds by feel rather than by typing: an opacity, a scale, a limit on a
 * bounded scale. The thumb rides a hairline track and the part below it fills in gray, so the
 * value reads as a proportion before it reads as a number; the number itself sits in the header
 * beside the label. Give it a pair of values and it grows a second thumb, for a span rather than
 * a point, and give it `marks` for a scale under the track.
 *
 * The thumb is inset so it sits inside the track at either end rather than hanging over it, the
 * way ⚛️ Core draws it. Under the pointer it widens and grows a grip, and the drag cursor
 * takes over while it is held (see DESIGN.md, Cursors).
 *
 * For a number the user knows and would rather type or nudge — a font size, a tab width — reach
 * for {@link NumberField}.
 *
 * @public
 * @import import { Range } from "@gitbutler/ui-react/Range.tsx";
 */
export const Range = <Value extends number | ReadonlyArray<number> = number>({
	label,
	value,
	defaultValue,
	onValueChange,
	onValueCommitted,
	min = 0,
	max = 100,
	step,
	largeStep,
	marks,
	format,
	disabled = false,
	name,
	className,
	style,
	"aria-label": ariaLabel,
}: RangeProps<Value>) => {
	const initial = value ?? defaultValue;
	const thumbCount = Array.isArray(initial) ? initial.length : 1;
	const formatter = new Intl.NumberFormat(undefined, format);
	// A thumb on a labelled mark reads that label to assistive tech, as it does on screen.
	const getAriaValueText = (formattedValue: string, thumbValue: number): string => {
		const mark = marks?.find((mark) => typeof mark !== "number" && mark.value === thumbValue);
		return mark !== undefined && typeof mark !== "number" && typeof mark.label === "string"
			? mark.label
			: formattedValue;
	};

	return (
		<Field.Root className={className} style={style}>
			<Slider.Root<Value>
				className={styles.range}
				value={value}
				defaultValue={defaultValue}
				onValueChange={onValueChange}
				onValueCommitted={onValueCommitted}
				min={min}
				max={max}
				step={step}
				largeStep={largeStep}
				format={format}
				disabled={disabled}
				name={name}
				thumbAlignment="edge"
			>
				{label !== undefined && (
					<div className={classes("text-12", styles.header)}>
						<Field.Label className={styles.label}>{label}</Field.Label>
						<Slider.Value className={styles.value} />
					</div>
				)}
				<Slider.Control className={styles.control}>
					<Slider.Track className={styles.track}>
						<Slider.Indicator className={styles.indicator} />
						{Array.from({ length: thumbCount }, (_, index) => (
							<Slider.Thumb
								key={index}
								index={index}
								aria-label={ariaLabel}
								getAriaValueText={getAriaValueText}
								// Base UI puts data-dragging on every thumb while any of them drags, so the
								// thumb in the hand is told apart from the other by the root's active index.
								className={(state) =>
									classes(
										styles.thumb,
										state.dragging && state.activeThumbIndex === index && styles.dragging,
									)
								}
							/>
						))}
					</Slider.Track>
				</Slider.Control>
				{marks !== undefined && (
					<div className={classes("text-12", styles.marks)} aria-hidden="true">
						{marks.map((mark) => {
							const value = typeof mark === "number" ? mark : mark.value;
							return (
								<span
									key={value}
									className={styles.mark}
									style={{ "--mark-position": (value - min) / (max - min) }}
								>
									{typeof mark === "number" ? formatter.format(mark) : mark.label}
								</span>
							);
						})}
					</div>
				)}
			</Slider.Root>
		</Field.Root>
	);
};
