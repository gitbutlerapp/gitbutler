import preview from "#storybook/preview";
import { useVirtualizer } from "@tanstack/react-virtual";
import { type ReactNode, useRef, useState } from "react";
import { Button } from "./Button.tsx";
import { Checkbox } from "./Checkbox.tsx";
import { FileList, FileListItem } from "./FileList.tsx";
import type { FileStatusType } from "./FileStatusBadge.tsx";
import { Icon } from "./Icon.tsx";

const statuses: Array<FileStatusType> = ["Addition", "Deletion", "Modification", "Rename"];

const meta = preview.meta({
	component: FileListItem,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=2341-2564",
		},
	},
	argTypes: {
		status: { control: "select", options: statuses },
		directoryPosition: { control: "inline-radio", options: ["trail", "lead"] },
	},
	args: {
		name: "userSlice.ts",
		directory: "src/store",
		status: "Modification",
		selected: false,
		reviewed: false,
		conflicted: false,
	},
	decorators: [
		(Story) => (
			<div style={{ width: 340, paddingBlock: 24 }}>
				<Story />
			</div>
		),
	],
});

/** One file: its type glyph, its name with the path after it, and the letter for its change. */
export const Default = meta.story({
	args: { name: "userSlice.ts" },
	render: (args) => (
		<FileList focused>
			<FileListItem {...args} />
		</FileList>
	),
});

const kebab = (
	<Button variant="ghost" size="small" iconOnly aria-label="File menu">
		<Icon name="kebab" />
	</Button>
);

/**
 * Every state a row can be in. The list holds the selection focus, so the selected row is solid;
 * the second list has lost it. A row that can't be acted on right now is `inert`.
 */
export const States = meta.story({
	render: () => (
		<div style={{ display: "flex", flexDirection: "column", gap: 24 }}>
			<FileList focused>
				<FileListItem name="useAuthHook.ts" directory="src/hooks" status="Addition" />
				<FileListItem
					selected
					name="Button.tsx"
					directory="src/components"
					status="Modification"
					actions={kebab}
				/>
				<FileListItem
					name="apiService.ts"
					directory="src/services"
					status="Modification"
					reviewed
				/>
				<FileListItem name="merge.rs" directory="crates/core" status="Modification" conflicted />
				<FileListItem name="index.ts" directory="src" status="Deletion" inert />
			</FileList>
			<FileList>
				<FileListItem name="styles.css" directory="src/styles" status="Addition" />
				<FileListItem selected name="theme.css" directory="src/styles" status="Modification" />
			</FileList>
		</div>
	),
});

type TreeNode =
	| { kind: "dir"; name: string; children: Array<TreeNode> }
	| { kind: "file"; name: string; status: FileStatusType };

const tree: Array<TreeNode> = [
	{
		kind: "dir",
		name: "src",
		children: [
			{
				kind: "dir",
				name: "components",
				children: [
					{ kind: "file", name: "Button.tsx", status: "Modification" },
					{ kind: "file", name: "Tooltip.tsx", status: "Addition" },
				],
			},
			{
				kind: "dir",
				name: "styles",
				children: [
					{ kind: "file", name: "theme.css", status: "Modification" },
					{ kind: "file", name: "reset.css", status: "Deletion" },
					{ kind: "file", name: "tokens.css", status: "Addition" },
				],
			},
			{ kind: "file", name: "index.ts", status: "Modification" },
		],
	},
	{ kind: "file", name: "package.json", status: "Modification" },
];

const countFiles = (node: TreeNode): number =>
	node.kind === "file" ? 1 : node.children.reduce((n, child) => n + countFiles(child), 0);

/**
 * A tree: each row steps in under its directory, with the guide that ties it there. A directory
 * folds on its chevron and says how many files it holds while folded; files under one leave out
 * their path, since the tree says it.
 */
export const Tree = meta.story({
	render: function Render() {
		const [folded, setFolded] = useState<ReadonlySet<string>>(() => new Set(["src/styles"]));
		const [checked, setChecked] = useState<ReadonlySet<string>>(() => new Set(["Button.tsx"]));
		const toggle = (set: ReadonlySet<string>, key: string) => {
			const next = new Set(set);
			if (!next.delete(key)) next.add(key);
			return next;
		};
		const rows = (nodes: Array<TreeNode>, depth: number, parent: string): Array<ReactNode> =>
			nodes.flatMap((node) => {
				const path = parent === "" ? node.name : `${parent}/${node.name}`;
				if (node.kind === "file") {
					return [
						<FileListItem
							key={path}
							role="treeitem"
							aria-level={depth + 1}
							name={node.name}
							depth={depth}
							status={node.status}
							checkbox={
								<Checkbox
									aria-label={`Check ${path}`}
									checked={checked.has(node.name)}
									onCheckedChange={() => setChecked((set) => toggle(set, node.name))}
								/>
							}
						/>,
					];
				}
				const isFolded = folded.has(path);
				return [
					<FileListItem
						key={path}
						role="treeitem"
						aria-level={depth + 1}
						aria-expanded={!isFolded}
						name={node.name}
						depth={depth}
						folded={isFolded}
						onToggleFolded={() => setFolded((set) => toggle(set, path))}
						count={isFolded ? countFiles(node) : undefined}
					/>,
					...(isFolded ? [] : rows(node.children, depth + 1, path)),
				];
			});
		return (
			<FileList focused role="tree" aria-label="Changes">
				{rows(tree, 0, "")}
			</FileList>
		);
	},
});

const many = Array.from({ length: 2000 }, (_, index) => ({
	name: `file-${index}.ts`,
	directory: `src/module-${Math.floor(index / 20)}`,
	status: statuses[index % statuses.length] ?? "Modification",
}));

/**
 * A long list, virtualised: the virtualizer positions each row and supplies the pixel between
 * them, since rows it places don't take the list's gap.
 */
export const Virtualised = meta.story({
	render: function Render() {
		const scrollRef = useRef<HTMLDivElement>(null);
		const [selected, setSelected] = useState(0);
		const virtualizer = useVirtualizer({
			count: many.length,
			getScrollElement: () => scrollRef.current,
			estimateSize: () => 28,
			gap: 1,
		});
		return (
			<div ref={scrollRef} style={{ height: 320, overflow: "auto" }}>
				<FileList focused>
					<div style={{ position: "relative", height: virtualizer.getTotalSize() }}>
						{virtualizer.getVirtualItems().map((row) => {
							const file = many[row.index];
							if (file === undefined) return null;
							return (
								<FileListItem
									key={row.key}
									selected={row.index === selected}
									{...file}
									onClick={() => setSelected(row.index)}
									data-index={row.index}
									ref={virtualizer.measureElement}
									style={{
										position: "absolute",
										insetInline: 0,
										transform: `translateY(${row.start}px)`,
									}}
								/>
							);
						})}
					</div>
				</FileList>
			</div>
		);
	},
});
