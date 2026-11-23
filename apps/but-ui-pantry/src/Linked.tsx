import { Button } from "@gitbutler/but-ui-react/Button.tsx";
import { Icon } from "@gitbutler/but-ui-react/Icon.tsx";
import { Tooltip } from "@gitbutler/but-ui-react/Tooltip.tsx";
import { Children, isValidElement, useRef, useState, type FC, type ReactNode } from "react";
import { anchorOf, reveal } from "./site.ts";

/** The page's address with `#id` on the end, its tab and all. */
const addressOf = (id: string) => {
	const address = new URL(window.location.href);
	address.hash = id;
	return address.toString();
};

/**
 * The address's block scrolls into view once, when it's first drawn, not on every render, and its
 * title is lit for a second so whoever opened the link sees which rule it meant.
 */
const scrollIfNamed = (id: string) => (node: HTMLElement | null) => {
	if (node === null || node.dataset.scrolled !== undefined) return;
	node.dataset.scrolled = "";
	if (decodeURIComponent(window.location.hash.slice(1)) === id) reveal(node);
};

/**
 * The link button in the margin before a heading or a paragraph: it copies the page's address with
 * the block's `#id` and says "Copied!" for a moment. It shows under the pointer or the keyboard.
 */
const CopyLink: FC<{ id: string }> = ({ id }) => {
	const [copied, setCopied] = useState(false);
	const reset = useRef<ReturnType<typeof setTimeout> | null>(null);

	const copy = () => {
		const address = addressOf(id);
		window.history.replaceState(null, "", address);
		// Says it's copied only once the clipboard took it; the address bar has it either way.
		void navigator.clipboard.writeText(address).then(
			() => {
				setCopied(true);
				if (reset.current !== null) clearTimeout(reset.current);
				reset.current = setTimeout(() => setCopied(false), 1500);
			},
			() => {},
		);
	};

	return (
		<span className="docs-linked-gutter">
			<Tooltip content={copied ? "Copied!" : "Copy link"}>
				<Button
					variant="ghost"
					size="small"
					iconOnly
					aria-label="Copy link to this section"
					className="docs-linked-button"
					onClick={copy}
				>
					<Icon name={copied ? "tick" : "link"} />
				</Button>
			</Tooltip>
		</span>
	);
};

/** A heading that can be linked to, with its link button in the margin. */
export const LinkedHeading: FC<{ id: string; className: string; children: ReactNode }> = ({
	id,
	className,
	children,
}) => (
	<h2 id={id} ref={scrollIfNamed(id)} className={`docs-linked ${className}`}>
		<CopyLink id={id} />
		<span className="docs-linked-title">{children}</span>
	</h2>
);

/** The words in a node, as a heading's or a bold lead's. */
const textOf = (node: ReactNode): string =>
	typeof node === "string" || typeof node === "number"
		? String(node)
		: isValidElement<{ children?: ReactNode }>(node)
			? textOf(node.props.children)
			: Array.isArray(node)
				? node.map(textOf).join("")
				: "";

/**
 * A design note's paragraph, for Markdown's `renderParagraph`. One that opens with a bold lead, as
 * a rule does ("**The buttons stand at the end,**"), can be linked to, by its lead's words; any
 * other is a plain paragraph.
 */
export const LinkedParagraph: FC<{ children: ReactNode }> = ({ children }) => {
	const [first] = Children.toArray(children);
	if (!isValidElement(first) || first.type !== "strong") return <p>{children}</p>;
	const id = anchorOf(textOf(first));
	return (
		<p id={id} ref={scrollIfNamed(id)} className="docs-linked">
			<CopyLink id={id} />
			{children}
		</p>
	);
};
