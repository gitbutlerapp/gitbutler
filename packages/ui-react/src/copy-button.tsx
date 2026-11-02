import { Button } from "./Button.tsx";
import { classes } from "./classes.ts";
import styles from "./copy-button.module.css";
import { Icon } from "./Icon.tsx";
import { Tooltip } from "./Tooltip.tsx";
import { useRef, useState, type FC } from "react";

/**
 * The copy button beside what `CodeBlock` and `DescriptionListItem` copy, so copying looks and
 * answers the same in both: a tick and "Copied!" for a moment after the press.
 */
export const CopyButton: FC<{
	/** The button's name and tooltip, in the imperative: "Copy commands". */
	label: string;
	onCopy: () => void;
	className?: string;
}> = ({ label, onCopy, className }) => {
	const [copied, setCopied] = useState(false);
	const reset = useRef<ReturnType<typeof setTimeout> | null>(null);

	const copy = () => {
		onCopy();
		setCopied(true);
		if (reset.current !== null) clearTimeout(reset.current);
		reset.current = setTimeout(() => setCopied(false), 1500);
	};

	return (
		<Tooltip content={copied ? "Copied!" : label}>
			<Button
				variant="ghost"
				size="small"
				iconOnly
				aria-label={label}
				className={classes(styles.copy, className)}
				onClick={copy}
			>
				<Icon name={copied ? "tick" : "copy"} />
			</Button>
		</Tooltip>
	);
};
