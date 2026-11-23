import {
	Children,
	cloneElement,
	isValidElement,
	type FC,
	type ReactElement,
	type ReactNode,
} from "react";
import { Button } from "../Button.tsx";
import { Icon } from "../Icon.tsx";
import { Tooltip } from "../Tooltip.tsx";
import {
	BranchItem,
	CommitItem,
	GroupLabel,
	MachineItem,
	MoreItem,
	NoticeItem,
	RepoItem,
	StackCaption,
	TerminalItem,
	UncommittedItem,
	UpstreamItem,
	WorktreeItem,
} from "../SidebarRow.tsx";
import { treeGuides, type Guides } from "../treeGuides.ts";

/*
 * What the Workspace tree's stories share: the row's buttons, and the card, column and repository
 * they stand in. Each page's own trees stay in its stories file.
 */
/* oxlint-disable react/only-export-components -- The rows' buttons and the machine row are elements the stories drop in as they are, beside the components that hold them. */

/** A count's next step, in the count's place: its number, then its glyph, in its colour. */
const step = (
	key: string,
	variant: "info" | "warning",
	glyph: "arrow-up" | "arrow-down",
	tip: string,
	n: number,
) => (
	<Tooltip key={key} content={tip}>
		<Button size="small" variant={variant} aria-label={tip}>
			{n}
			<Icon name={glyph} />
		</Button>
	</Tooltip>
);

const commits = (n: number) => `${n} commit${n === 1 ? "" : "s"}`;

/** Push, in place of ↑ `unpushed`. */
export const push = (n: number) => step("push", "info", "arrow-up", `Push ${commits(n)}`, n);

/** Update, in place of ↓ `behind`. */
export const update = (n: number, from = "the target") =>
	step("update", "warning", "arrow-down", `Update with ${commits(n)} from ${from}`, n);

/**
 * A sidebar card: a machine's header over its repositories, as the Workspace tree stories build them. A
 * section of the page, named for its machine, so a screen reader can move between cards.
 */
export const Card: FC<{
	children: ReactNode;
	focused?: boolean;
	header: ReactNode;
	label: string;
}> = ({ children, focused = false, header, label }) => (
	<section
		aria-label={label}
		style={{
			width: 334,
			flexShrink: 0,
			overflow: "hidden",
			border: "1px solid var(--but-border-subtle)",
			borderRadius: "var(--but-radius-card)",
			backgroundColor: "var(--but-background)",
		}}
		data-selection-focused={focused ? "true" : undefined}
	>
		<div style={{ padding: 8, borderBottom: "1px solid var(--but-border-subtle)" }}>{header}</div>
		{children}
	</section>
);

const rowTypes = new Set<unknown>([
	MachineItem,
	RepoItem,
	WorktreeItem,
	UncommittedItem,
	StackCaption,
	BranchItem,
	CommitItem,
	UpstreamItem,
	MoreItem,
	NoticeItem,
	TerminalItem,
]);

/** Gives the rows in `children` their guides, worked out with `treeGuides` from the rows in the
 * order they draw, as a host does from the tree it shows. */
const guided = (children: ReactNode): ReactNode => {
	const walk = (node: ReactNode, visit: (row: ReactElement) => ReactNode): ReactNode =>
		Children.map(node, (child) => {
			if (!isValidElement<{ children?: ReactNode }>(child)) return child;
			if (rowTypes.has(child.type)) return visit(child);
			if (child.props.children === undefined) return child;
			return cloneElement(child, undefined, walk(child.props.children, visit));
		});
	const rows: Array<{ depth: number }> = [];
	void walk(children, (row) => {
		const { depth = 0 } = row.props as { depth?: number };
		rows.push({ depth });
		return row;
	});
	const guides = treeGuides(rows);
	let next = 0;
	return walk(children, (row) =>
		cloneElement(row as ReactElement<{ guides?: Guides }>, { guides: guides[next++] }),
	);
};

/** Rows one after another, as a sidebar lists them. */
export const Column: FC<{ children: ReactNode }> = ({ children }) => (
	<div style={{ display: "flex", flexDirection: "column", gap: 1 }}>{guided(children)}</div>
);

/** One repository's rows in a card, ruled off from the next. */
export const Repository: FC<{ children: ReactNode; last?: boolean }> = ({
	children,
	last = false,
}) => (
	<div
		style={{
			display: "flex",
			flexDirection: "column",
			gap: 1,
			padding: 8,
			borderBottom: last ? undefined : "1px solid var(--but-border-subtle)",
		}}
	>
		{guided(children)}
	</div>
);

type Terminal = { name: string; place: string; meta?: ReactNode };

/** Terminals under their status's label in a card, ruled off from the next status, on the guide
 * that ties them to the label's chevron. */
export const TerminalGroup: FC<{ label: string; terminals: Array<Terminal>; last?: boolean }> = ({
	label,
	terminals,
	last = false,
}) => {
	const guides = treeGuides([{ depth: 0 }, ...terminals.map(() => ({ depth: 1 }))]);
	return (
		<div
			style={{
				display: "flex",
				flexDirection: "column",
				gap: 1,
				padding: 8,
				borderBottom: last ? undefined : "1px solid var(--but-border-subtle)",
			}}
		>
			<GroupLabel name={label} folded={false} />
			{terminals.map((terminal, index) => (
				<TerminalItem key={terminal.name} {...terminal} depth={1} guides={guides[index + 1]} />
			))}
		</div>
	);
};

export const machine = (
	<MachineItem name="pavel--macbook-pro-2" online folded={false} icon={<Icon name="laptop" />} />
);
