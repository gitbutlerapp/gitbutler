import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import type { IconName } from "#ui/components/iconNames.ts";
import * as md from "#ui/markdown-editing.ts";
import { applyToTextarea } from "#ui/markdown-textarea.ts";
import { Tooltip } from "@base-ui/react";
import type { FC, RefObject } from "react";
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
 */
export const MarkdownToolbar: FC<Props> = (p) => {
	const apply = (command: md.MarkdownCommand) => {
		const target = p.targetRef.current;
		if (target !== null) p.onInput(applyToTextarea(target, command));
	};

	return (
		<div className={classes(p.className, styles.toolbar)} role="toolbar" aria-label="Formatting">
			{groups.map((group, index) => (
				// Indices are stable: the groups are a module constant.
				// oxlint-disable-next-line react/no-array-index-key
				<div key={index} className={styles.group}>
					{index > 0 && <div aria-hidden className={styles.separator} />}
					{group.map((button) => (
						<Tooltip.Root key={button.label}>
							<Tooltip.Trigger
								className={getButtonClassName({ variant: "ghost", iconOnly: true })}
								// base-ui's own `disabled` only suppresses the tooltip, leaving a
								// live button behind, so the native attributes go on the element.
								render={<button aria-label={button.label} disabled={p.disabled} type="button" />}
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
			))}
		</div>
	);
};
