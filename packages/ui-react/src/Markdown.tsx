import { compiler } from "markdown-to-jsx";
import {
	Children,
	isValidElement,
	type ComponentProps,
	type CSSProperties,
	type FC,
	type ReactElement,
	type ReactNode,
} from "react";
import { Checkbox } from "./Checkbox.tsx";
import { classes } from "./classes.ts";
import { Code } from "./Code.tsx";
import { CodeBlock } from "./CodeBlock.tsx";
import styles from "./Markdown.module.css";
import { Table, TableCell, TableRow } from "./Table.tsx";
import { TextLink } from "./TextLink.tsx";

type LinkRenderer = (href: string, children: ReactNode) => ReactNode;

/**
 * Markdown as it is written on GitHub, tables and task lists included: a pull request description,
 * a comment, a design note. It is drawn with the library's parts: headings in design-core's
 * `text-h1` to `text-h3`, a link as a `TextLink`, inline code as `Code`, a fenced block as a
 * `CodeBlock`, a table as a `Table`, a task as a read-only `Checkbox`. Text reads in a
 * `--size-prose` column; tables and images take `--size-prose-wide`, centred on it. The rules are
 * `design/patterns/markdown.md`.
 *
 * HTML in the text is shown as text rather than run, and a link to an unsafe protocol loses its
 * address, so markdown from an agent or a stranger can't run a script.
 *
 * Every link leaves the app, so each is a `TextLink` that opens in a new tab. How a link leaves is
 * the host's to say, through `renderLink`, which draws one from its address and text: Electron hands
 * the URL to the system browser, and a client can show a path on another machine as plain text.
 * With `onCopy`, a fenced block has the copy button `CodeBlock` has.
 *
 * @public
 * @import import { Markdown } from "@gitbutler/ui-react/Markdown.tsx";
 */
export const Markdown: FC<{
	/** The markdown source. */
	children: string;
	renderLink?: LinkRenderer;
	onCopy?: (code: string) => void;
	className?: string;
}> = ({ children, renderLink = openInNewTab, onCopy, className }) => (
	<div className={classes("text-13", "text-body", styles.markdown, className)}>
		{/* The compiler rather than its component: the blocks go straight into this element, and the
		    component would wrap them in one of its own. */}
		{compiler(children, {
			wrapper: null,
			forceBlock: true,
			disableParsingRawHTML: true,
			overrides: {
				h1: { props: { className: "text-h1" } },
				h2: { props: { className: "text-h2" } },
				h3: { props: { className: "text-h3" } },
				h4: { props: { className: "text-semibold" } },
				h5: { props: { className: "text-semibold" } },
				h6: { props: { className: "text-semibold" } },
				a: { component: Link, props: { renderLink } },
				code: Code,
				pre: { component: Pre, props: { onCopy } },
				table: MarkdownTable,
				tr: TableRow,
				th: { component: Cell, props: { header: true } },
				td: Cell,
				input: Task,
				img: { props: { className: styles.image } },
			},
		})}
	</div>
);

const openInNewTab: LinkRenderer = (href, children) => (
	<TextLink href={href} target="_blank" rel="noreferrer">
		{children}
	</TextLink>
);

/** A link the sanitizer emptied, an unsafe one, keeps its text without an address. */
const Link: FC<ComponentProps<"a"> & { renderLink: LinkRenderer }> = ({
	href,
	children,
	renderLink,
}) => (href === undefined || href === "" ? children : renderLink(href, children));

/** A fenced block: its one `code` child holds the text. */
const Pre: FC<{ children?: ReactNode; onCopy?: (code: string) => void }> = ({
	children,
	onCopy,
}) => {
	const code = isValidElement<{ children?: ReactNode }>(children) ? children.props.children : "";
	return (
		<CodeBlock onCopy={onCopy} className={styles.box}>
			{(typeof code === "string" ? code : "").replace(/\n$/, "")}
		</CodeBlock>
	);
};

/** The parser's `thead` and `tbody`, unwrapped into the Table's header and rows. */
const MarkdownTable: FC<{ children?: ReactNode }> = ({ children }) => {
	const [head, body] = Children.toArray(children) as Array<ReactElement<{ children?: ReactNode }>>;
	return (
		<Table header={head?.props.children} className={classes(styles.box, styles.wide)}>
			{body?.props.children}
		</Table>
	);
};

/** A column's alignment arrives as an inline style; numbers aligned right line up at the end. */
const Cell: FC<{ header?: boolean; style?: CSSProperties; children?: ReactNode }> = ({
	header,
	style,
	children,
}) => (
	<TableCell header={header} align={style?.textAlign === "right" ? "end" : "start"}>
		{children}
	</TableCell>
);

/** A task list's box: shows whether the task is done, and can't be ticked here. */
const Task: FC<ComponentProps<"input">> = ({ type, checked }) =>
	type === "checkbox" ? (
		<Checkbox checked={checked ?? false} readOnly className={styles.task} />
	) : null;
