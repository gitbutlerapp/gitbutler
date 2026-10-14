import styles from "./CopyableId.module.css";
import { Icon } from "./Icon.tsx";
import type { IconName } from "./iconNames.ts";
import { Tooltip } from "./Tooltip.tsx";
import { useRef, useState, type FC } from "react";

/**
 * An ID that copies itself when pressed: a commit's or a change's, in a meta line. It shows the
 * short form and copies the whole one, and says "Copied!" in its place for a moment.
 *
 * The copying is the host's, through `onCopy`, since the library doesn't know the clipboard: Lite
 * hands the value to Electron, a web page to `navigator.clipboard`.
 * @import import { CopyableId } from "@gitbutler/ui-react/CopyableId.tsx";
 */
export const CopyableId: FC<{
	/** What is copied, for the tooltip and the button's name: "Copy commit ID". */
	label: string;
	/** `hash` for a commit ID, `finger-print` for a change ID. */
	icon: IconName;
	/** The whole ID, which is what is copied. */
	value: string;
	/** The short form shown; the whole `value` when omitted. */
	display?: string;
	onCopy: (value: string) => void;
}> = ({ label, icon, value, display = value, onCopy }) => {
	const [copied, setCopied] = useState(false);
	const reset = useRef<ReturnType<typeof setTimeout> | null>(null);

	const copy = () => {
		onCopy(value);
		setCopied(true);
		if (reset.current !== null) clearTimeout(reset.current);
		reset.current = setTimeout(() => setCopied(false), 1500);
	};

	return (
		<Tooltip content={label}>
			<button type="button" aria-label={label} className={styles.id} onClick={copy}>
				<Icon size={14} name={copied ? "tick" : icon} />
				<span>{copied ? "Copied!" : display}</span>
			</button>
		</Tooltip>
	);
};
