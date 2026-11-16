import { stories } from "../stories/stories.ts";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useRef, useState, type MouseEvent } from "react";
import { Button } from "./Button.tsx";
import { Checkbox } from "./Checkbox.tsx";
import { FileList, FileListItem } from "./FileList.tsx";
import type { FileStatusType } from "./FileStatusBadge.tsx";
import { Icon } from "./Icon.tsx";
import { ContextMenu, MenuItem, MenuSection } from "./Menu.tsx";
import { Snackbar } from "./Snackbar.tsx";
import { treeGuides } from "./treeGuides.ts";

const statuses: Array<FileStatusType> = ["Addition", "Deletion", "Modification", "Rename"];

const meta = stories({
	title: "Files & diffs/FileList",
	id: "components-filelist",
	component: FileListItem,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=2341-2564",
		},
		width: 340,
	},
	args: {
		name: "userSlice.ts",
		directory: "src/store",
		status: "Modification",
		added: 12,
		removed: 3,
		selected: false,
		reviewed: false,
		conflicted: false,
	},
});

/**
 * One file in a panel: the header names the files with their count and line totals, and the row
 * gives the file's type glyph, its name with the path after it, its line counts, and the letter
 * for its change.
 */
export const Default = meta.story({
	args: { name: "userSlice.ts" },
	render: (args) => (
		<FileList title="Changes" count={1} added={12} removed={3} focused>
			<FileListItem {...args} />
		</FileList>
	),
});

/** A file's actions, from a right-click on its row: a file row has no ⋯. */
const fileActions = (
	<>
		<MenuSection>
			<MenuItem>Open in editor</MenuItem>
			<MenuItem>Copy path</MenuItem>
		</MenuSection>
		<MenuSection>
			<MenuItem>Discard changes</MenuItem>
		</MenuSection>
	</>
);

/**
 * Every state a row can be in. The list holds the selection focus, so the selected row is solid;
 * the second list has lost it. A row that can't be acted on right now, or whose action is running,
 * is `inert`. A row's menu opens with a
 * right-click, as on the selected one: a file row has no ⋯, so its last cell always says what
 * happened.
 */
export const States = meta.story({
	render: () => (
		<div style={{ display: "flex", flexDirection: "column", gap: 24 }}>
			<FileList title="Changes" count={5} added={120} removed={48} focused>
				<FileListItem name="useAuthHook.ts" directory="src/hooks" status="Addition" added={64} />
				<ContextMenu
					aria-label="File actions"
					trigger={
						<FileListItem
							selected
							name="Button.tsx"
							directory="src/components"
							status="Modification"
							added={18}
							removed={9}
						/>
					}
				>
					{fileActions}
				</ContextMenu>
				<FileListItem
					name="apiService.ts"
					directory="src/services"
					status="Modification"
					added={4}
					removed={2}
					reviewed
				/>
				<FileListItem
					name="merge.rs"
					directory="crates/core"
					status="Modification"
					added={34}
					removed={37}
					conflicted
				/>
				<FileListItem name="index.ts" directory="src" status="Deletion" inert />
			</FileList>
			<FileList title="Changes" count={2} added={30}>
				<FileListItem name="styles.css" directory="src/styles" status="Addition" />
				<FileListItem selected name="theme.css" directory="src/styles" status="Modification" />
			</FileList>
		</div>
	),
});

const changes: Array<{ name: string; directory: string; status: FileStatusType }> = [
	{ name: "useAuthHook.ts", directory: "src/hooks", status: "Addition" },
	{ name: "Button.tsx", directory: "src/components", status: "Modification" },
	{ name: "apiService.ts", directory: "src/services", status: "Modification" },
	{ name: "routes.ts", directory: "src", status: "Modification" },
	{ name: "theme.css", directory: "src/styles", status: "Modification" },
];
const discarding = new Set(["Button.tsx", "apiService.ts", "routes.ts"]);

const plural = (count: number) => `${count} ${count === 1 ? "file" : "files"}`;

/**
 * Picking files and acting on them: click picks a file, ⌘-click (Ctrl-click) adds or takes one
 * away, and Shift-click picks the run from the last one clicked. Picked rows next to one another
 * read as one block. Discard works on what's picked: the rows go `inert`, faded and out of reach,
 * with no spinner of their own, and one snackbar counts them ("Discarding 3 files…") and turns
 * into the result as the rows leave. In an app the snackbar floats at the window's foot
 * (`Snackbars`); here it's drawn under the list.
 */
export const Discarding = meta.story({
	render: function Render() {
		const [shown, setShown] = useState(changes);
		const [picked, setPicked] = useState<ReadonlySet<string>>(() => new Set(discarding));
		const [anchor, setAnchor] = useState<string | null>(null);
		const [working, setWorking] = useState<ReadonlySet<string>>(() => new Set());
		// What the snackbar says, drawn under the list rather than floating at the window's foot.
		const [notice, setNotice] = useState<{ done: boolean; text: string } | null>(null);

		const order = shown.map((file) => file.name);

		/** A click on `name`. */
		const pick = (name: string, event: MouseEvent) => {
			if (event.shiftKey && anchor !== null && order.includes(anchor)) {
				const [a, b] = [order.indexOf(anchor), order.indexOf(name)];
				setPicked(new Set(order.slice(Math.min(a, b), Math.max(a, b) + 1)));
				return;
			}
			setAnchor(name);
			if (event.metaKey || event.ctrlKey) {
				setPicked((set) => {
					const next = new Set(set);
					if (!next.delete(name)) next.add(name);
					return next;
				});
			} else {
				setPicked(new Set([name]));
			}
		};
		// Shift-click picks a run; it shouldn't select the text in between too.
		const noTextSelection = (event: MouseEvent) => {
			if (event.shiftKey) event.preventDefault();
		};

		const discard = () => {
			const files = new Set(picked);
			const count = plural(files.size);
			setWorking(files);
			setPicked(new Set());
			setNotice({ done: false, text: `Discarding ${count}…` });
			setTimeout(() => {
				const result = `${count} discarded`;
				setWorking(new Set());
				setShown((rows) => rows.filter((file) => !files.has(file.name)));
				setNotice({ done: true, text: result });
				setTimeout(() => setNotice((now) => (now?.text === result ? null : now)), 5000);
			}, 2500);
		};

		return (
			<div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
				<div style={{ display: "flex", gap: 8 }}>
					<Button
						variant="danger"
						onClick={discard}
						disabled={working.size > 0 || picked.size === 0}
					>
						{picked.size === 0 ? "Discard" : `Discard ${plural(picked.size)}`}
					</Button>
					<Button
						variant="outline"
						onClick={() => {
							setShown(changes);
							setPicked(new Set(discarding));
							setNotice(null);
						}}
						disabled={working.size > 0}
					>
						Reset
					</Button>
				</div>
				<FileList title="Changes" count={shown.length} focused>
					{shown.map((file) => (
						<FileListItem
							key={file.name}
							{...file}
							selected={picked.has(file.name)}
							inert={working.has(file.name)}
							onClick={(event: MouseEvent) => pick(file.name, event)}
							onMouseDown={noTextSelection}
						/>
					))}
				</FileList>
				{notice !== null && (
					<Snackbar
						variant={notice.done ? "safe" : "info"}
						icon={notice.done ? undefined : "spinner"}
						style={{ alignSelf: "center" }}
					>
						{notice.text}
					</Snackbar>
				)}
			</div>
		);
	},
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

const filePaths = (node: TreeNode, path: string): Array<string> =>
	node.kind === "file"
		? [path]
		: node.children.flatMap((child) => filePaths(child, `${path}/${child.name}`));

/**
 * A tree: each row steps in under its directory, with the guide that ties it there. A directory
 * folds on its chevron and says how many files it holds while folded; files under one leave out
 * their path, since the tree says it. A directory's checkbox checks every file under it, and shows
 * indeterminate while only some are.
 */
export const Tree = meta.story({
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=2344-2502",
		},
	},
	render: function Render() {
		const [folded, setFolded] = useState<ReadonlySet<string>>(() => new Set(["src/styles"]));
		const [checked, setChecked] = useState<ReadonlySet<string>>(
			() => new Set(["src/components/Button.tsx"]),
		);
		const toggle = (set: ReadonlySet<string>, key: string) => {
			const next = new Set(set);
			if (!next.delete(key)) next.add(key);
			return next;
		};
		// The rows as they draw, skipping what is folded, so their guides come from `treeGuides`.
		const shown: Array<{ node: TreeNode; depth: number; path: string }> = [];
		const visit = (nodes: Array<TreeNode>, depth: number, parent: string) => {
			for (const node of nodes) {
				const path = parent === "" ? node.name : `${parent}/${node.name}`;
				shown.push({ node, depth, path });
				if (node.kind !== "file" && !folded.has(path)) visit(node.children, depth + 1, path);
			}
		};
		visit(tree, 0, "");
		const guides = treeGuides(shown.map(({ depth }) => ({ depth })));
		const checkAll = (paths: Array<string>, on: boolean) =>
			setChecked((set) => {
				const next = new Set(set);
				for (const path of paths) {
					if (on) next.add(path);
					else next.delete(path);
				}
				return next;
			});
		const rows = shown.map(({ node, depth, path }, index) => {
			if (node.kind === "file") {
				return (
					<FileListItem
						key={path}
						role="treeitem"
						aria-level={depth + 1}
						name={node.name}
						depth={depth}
						guides={guides[index]}
						status={node.status}
						checkbox={
							<Checkbox
								aria-label={`Check ${path}`}
								checked={checked.has(path)}
								onCheckedChange={() => setChecked((set) => toggle(set, path))}
							/>
						}
					/>
				);
			}
			const files = filePaths(node, path);
			const done = files.filter((file) => checked.has(file)).length;
			return (
				<FileListItem
					key={path}
					role="treeitem"
					aria-level={depth + 1}
					aria-expanded={!folded.has(path)}
					name={node.name}
					depth={depth}
					guides={guides[index]}
					folded={folded.has(path)}
					onToggleFolded={() => setFolded((set) => toggle(set, path))}
					count={folded.has(path) ? files.length : undefined}
					checkbox={
						<Checkbox
							aria-label={`Check ${path}`}
							checked={done === files.length}
							indeterminate={done > 0 && done < files.length}
							onCheckedChange={() => checkAll(files, done < files.length)}
						/>
					}
				/>
			);
		});
		return (
			<FileList title="Changes" count={6} added={214} removed={37} focused>
				<div role="tree" aria-label="Changes" style={{ display: "contents" }}>
					{rows}
				</div>
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
			<div style={{ display: "flex", flexDirection: "column", height: 360 }}>
				<FileList
					title="Changes"
					count={many.length}
					added={48210}
					removed={9021}
					focused
					viewportRef={scrollRef}
					style={{ flexGrow: 1 }}
				>
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

const filterable = [
	{ name: "useAuthHook.ts", directory: "src/hooks", status: "Addition" },
	{ name: "Button.tsx", directory: "src/components", status: "Modification" },
	{ name: "Tooltip.tsx", directory: "src/components", status: "Modification" },
	{ name: "theme.css", directory: "src/styles", status: "Modification" },
	{ name: "index.ts", directory: "src", status: "Deletion" },
] satisfies Array<{ name: string; directory: string; status: FileStatusType }>;

/**
 * The search button turns the header into the filter field in place, so narrowing the list costs
 * no height; the cross or Escape brings the header back.
 */
export const Filter = meta.story({
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=2350-2896",
		},
	},
	render: function Render() {
		const [query, setQuery] = useState<string | null>("comp");
		const shown = filterable.filter(
			(file) => query === null || `${file.directory}/${file.name}`.includes(query),
		);
		return (
			<FileList
				title="Changes"
				count={filterable.length}
				added={120}
				removed={48}
				focused
				onOpenFilter={() => setQuery("")}
				filter={
					query === null
						? null
						: { value: query, onChange: setQuery, onClose: () => setQuery(null) }
				}
				actions={
					<Button variant="ghost" iconOnly aria-label="Changes menu">
						<Icon name="kebab" />
					</Button>
				}
			>
				{shown.map((file) => (
					<FileListItem key={file.name} {...file} />
				))}
			</FileList>
		);
	},
});
