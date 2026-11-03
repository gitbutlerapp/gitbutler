import { classes } from "./classes.ts";
import styles from "./CodeBlock.module.css";
import { CopyButton } from "./copy-button.tsx";
import { ScrollArea } from "./ScrollArea.tsx";
import { Fragment, type FC } from "react";

/**
 * Code on lines of its own: commands to run one after another, a snippet to paste. It reads as
 * Markdown's fenced code block does. For one command inside a sentence, use `Code`.
 *
 * `prompt` marks each line as a shell command with a `$`. The prompt is drawn, not part of the
 * text, so neither the copy button nor a selection by hand takes it along. A line too long for the
 * block scrolls sideways rather than wrapping, so it stays one command.
 *
 * With `onCopy`, a copy button sits beside the first line and copies the whole block; it stays in
 * view, since copying is usually why the block is there. The copying is the host's, as
 * `CopyableId`'s is.
 *
 * @public
 * @import import { CodeBlock } from "@gitbutler/ui-react/CodeBlock.tsx";
 */
export const CodeBlock: FC<{
	/** The code, one command to a line. */
	children: string;
	/** Draws a `$` before each line, for commands to run in a terminal. */
	prompt?: boolean;
	onCopy?: (code: string) => void;
	className?: string;
}> = ({ children, prompt = false, onCopy, className }) => {
	const label = prompt ? (children.includes("\n") ? "Copy commands" : "Copy command") : "Copy code";

	return (
		<div className={classes(className, styles.block)}>
			<ScrollArea className={styles.scroll} viewportClassName={styles.viewport}>
				<pre className={styles.code} data-prompt={prompt || undefined}>
					<code>
						{children.split("\n").map((line, index) => (
							// oxlint-disable-next-line react/no-array-index-key -- Lines repeat; their order is their identity.
							<Fragment key={index}>
								{index > 0 && "\n"}
								<span className={styles.line}>{line}</span>
							</Fragment>
						))}
					</code>
				</pre>
			</ScrollArea>
			{onCopy && (
				<CopyButton label={label} onCopy={() => onCopy(children)} className={styles.copy} />
			)}
		</div>
	);
};
