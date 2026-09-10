import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import type { IconName } from "#ui/components/iconNames.ts";
import * as md from "#ui/markdown-editing.ts";
import { applyToTextarea } from "#ui/markdown-textarea.ts";
import { Tooltip } from "@base-ui/react";
import { Fragment, useRef, useState } from "react";
import type { FC, RefCallback, RefObject } from "react";
import styles from "./MarkdownToolbar.module.css";

type ToolbarButton = {
	icon: IconName;
	label: string;
	command: md.MarkdownCommand;
};

/** Groups render separated by a rule, matching the designed toolbar. */
const groups: ReadonlyArray<ReadonlyArray<ToolbarButton>> = [
	[
		{ icon: "bullet-list", label: "Bulleted list", command: md.bulletList },
		{ icon: "number-list", label: "Numbered list", command: md.numberList },
		{ icon: "checklist", label: "Task list", command: md.taskList },
	],
	[
		{ icon: "text-bold", label: "Bold", command: md.bold },
		{ icon: "text-italic", label: "Italic", command: md.italic },
		{ icon: "text-strikethrough", label: "Strikethrough", command: md.strikethrough },
	],
	[
		{ icon: "text-code", label: "Code", command: md.code },
		{ icon: "text-quote", label: "Quote", command: md.quote },
		{ icon: "link", label: "Link", command: md.link },
	],
	[
		{ icon: "text-plain", label: "Plain text", command: md.plainText },
		{ icon: "text-h2", label: "Heading 2", command: md.heading2 },
		{ icon: "text-h3", label: "Heading 3", command: md.heading3 },
	],
];

/** Where the strip's scroll position sits; `fits` means there is nothing to scroll. */
type Reach = "fits" | "start" | "middle" | "end";

const measureReach = (strip: HTMLElement): Reach => {
	// The chevrons sit beside the strip and take room from it, so whether the
	// buttons fit is judged on the room they would give back once gone;
	// otherwise showing them could never be undone by widening.
	const nav = strip.nextElementSibling;
	const room =
		strip.clientWidth +
		(nav === null ? 0 : nav.getBoundingClientRect().right - strip.getBoundingClientRect().right);
	if (strip.scrollWidth <= room + 1) return "fits";
	const max = strip.scrollWidth - strip.clientWidth;
	if (strip.scrollLeft <= 1) return "start";
	if (strip.scrollLeft >= max - 1) return "end";
	return "middle";
};

type Props = {
	/** The textarea whose markdown source the buttons rewrite. */
	targetRef: RefObject<HTMLTextAreaElement | null>;
	/** Receives the rewritten source, for the owner's controlled state. */
	onInput: (value: string) => void;
	disabled?: boolean;
	className?: string;
};

/**
 * Markdown formatting buttons for a plain textarea. The commands themselves
 * live in `markdown-editing.ts`; this only routes them at the live selection.
 *
 * Too narrow to show every group, the buttons scroll sideways a group at a
 * time, with a chevron pair at the end for anyone without a horizontal wheel.
 */
export const MarkdownToolbar: FC<Props> = (p) => {
	const [reach, setReach] = useState<Reach>("fits");
	const stripRef = useRef<HTMLDivElement | null>(null);

	// A ref callback rather than an effect: the strip's width is what decides
	// whether the chevrons show, so it is measured as soon as it exists.
	const observeStrip: RefCallback<HTMLDivElement> = (strip) => {
		stripRef.current = strip;
		if (strip === null) return;
		const measure = () => setReach(measureReach(strip));
		measure();
		const observer = new ResizeObserver(measure);
		observer.observe(strip);
		return () => observer.disconnect();
	};

	const apply = (command: md.MarkdownCommand) => {
		const target = p.targetRef.current;
		if (target !== null) p.onInput(applyToTextarea(target, command));
	};

	const scrollByGroup = (direction: -1 | 1) => {
		const strip = stripRef.current;
		if (strip === null) return;
		const starts = Array.from(strip.querySelectorAll<HTMLElement>("[data-group]")).map(
			(group) => group.offsetLeft,
		);
		const current = strip.scrollLeft;
		const next =
			direction > 0
				? starts.find((start) => start > current + 1)
				: starts.findLast((start) => start < current - 1);
		if (next !== undefined) strip.scrollTo({ left: next, behavior: "smooth" });
	};

	return (
		<div className={classes(p.className, styles.toolbar)} role="toolbar" aria-label="Formatting">
			<div
				className={styles.strip}
				data-reach={reach}
				onScroll={(evt) => setReach(measureReach(evt.currentTarget))}
				ref={observeStrip}
			>
				{groups.map((group, index) => (
					// Indices are stable: the groups are a module constant.
					// oxlint-disable-next-line react/no-array-index-key
					<Fragment key={index}>
						{index > 0 && <div aria-hidden className={styles.separator} />}
						<div className={styles.group} data-group>
							{group.map((button) => (
								<Tooltip.Root key={button.label}>
									<Tooltip.Trigger
										className={getButtonClassName({ variant: "ghost", iconOnly: true })}
										// base-ui's own `disabled` only suppresses the tooltip, leaving a
										// live button behind, so the native attributes go on the element.
										render={
											<button aria-label={button.label} disabled={p.disabled} type="button" />
										}
										// Keeps the caret in the textarea: a plain click would blur it
										// first, so the command would have no selection to act on.
										onMouseDown={(evt) => evt.preventDefault()}
										onClick={() => apply(button.command)}
									>
										<Icon name={button.icon} />
									</Tooltip.Trigger>
									<Tooltip.Portal>
										<Tooltip.Positioner sideOffset={4}>
											<Tooltip.Popup render={<TooltipPopup />}>{button.label}</Tooltip.Popup>
										</Tooltip.Positioner>
									</Tooltip.Portal>
								</Tooltip.Root>
							))}
						</div>
					</Fragment>
				))}
			</div>

			{reach !== "fits" && (
				<div className={styles.nav}>
					<div aria-hidden className={styles.separator} />
					<button
						aria-label="Previous formatting group"
						className={getButtonClassName({ variant: "ghost", iconOnly: true })}
						disabled={reach === "start"}
						onMouseDown={(evt) => evt.preventDefault()}
						onClick={() => scrollByGroup(-1)}
						type="button"
					>
						<Icon name="chevron-left" />
					</button>
					<button
						aria-label="Next formatting group"
						className={getButtonClassName({ variant: "ghost", iconOnly: true })}
						disabled={reach === "end"}
						onMouseDown={(evt) => evt.preventDefault()}
						onClick={() => scrollByGroup(1)}
						type="button"
					>
						<Icon name="chevron-right" />
					</button>
				</div>
			)}
		</div>
	);
};
