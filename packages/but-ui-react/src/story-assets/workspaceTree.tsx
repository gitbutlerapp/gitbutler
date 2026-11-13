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
import {
	BranchItem,
	CommitItem,
	MachineItem,
	MoreItem,
	NoticeItem,
	RepoItem,
	StackCaption,
	TargetItem,
	UncommittedItem,
	WorktreeItem,
} from "../SidebarRow.tsx";
import { treeGuides, type Guides } from "../treeGuides.ts";

/*
 * What the Workspace tree's stories share: the row's buttons, and the card, column and repository
 * they stand in. Each page's own trees stay in its stories file.
 */
/* oxlint-disable react/only-export-components -- The rows' buttons and the machine row are elements the stories drop in as they are, beside the components that hold them. */

export const push = (
	<Button size="small" key="push">
		Push
	</Button>
);
export const update = (
	<Button size="small" key="update">
		Update
	</Button>
);
export const viewPr = (
	<Button size="small" iconOnly aria-label="Open pull request" key="pr">
		<Icon name="arrow-up-right" />
	</Button>
);
export const menu = (what: string) => (
	<Button variant="ghost" size="small" iconOnly aria-label={`${what} menu`}>
		<Icon name="kebab" />
	</Button>
);

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
	TargetItem,
	WorktreeItem,
	UncommittedItem,
	StackCaption,
	BranchItem,
	CommitItem,
	MoreItem,
	NoticeItem,
]);
/** The git objects among them, which meet their guide with a stub. */
const chainedTypes = new Set<unknown>([StackCaption, BranchItem, CommitItem]);

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
	const rows: Array<{ depth: number; chained: boolean }> = [];
	void walk(children, (row) => {
		const { depth = 0 } = row.props as { depth?: number };
		rows.push({ depth, chained: chainedTypes.has(row.type) });
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

export const machine = (
	<MachineItem
		name="pavel--macbook-pro-2"
		online
		folded={false}
		icon={<Icon name="laptop" />}
		menu={menu("Machine")}
	/>
);
