import { useState, type FC } from "react";
import { stories } from "../stories/stories.ts";
import { MetaCount, MetaStatus, type MetaStatusVariant } from "./MetaCount.tsx";
import {
	BranchItem,
	RepoItem,
	TerminalItem,
	UncommittedItem,
	WorktreeItem,
} from "./SidebarRow.tsx";
import { SplitPreview, SplitPreviewMap } from "./SplitPreview.tsx";
import { tileIds, type Tiling } from "./splitTiling.ts";
import { Card, machine, Repository } from "./story-assets/workspaceTree.tsx";

const butUi = (node: string) =>
	`https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=${node}`;

const status = (variant: MetaStatusVariant, tip: string) => (
	<MetaStatus variant={variant} aria-label={tip} />
);

const waiting = status("warning", "Waiting for you");
const shell = "pavellaptev@Pavels-MacBook-Pro:~/projects/shop";

/** A branch's page beside the agent that works on it and the tests it runs. */
const three: Tiling = {
	split: "across",
	parts: [
		{ id: "sales-tax", name: "sales-tax", icon: "branch", home: true },
		{
			split: "down",
			parts: [
				{ id: "claude", name: "claude", icon: "terminal", status: waiting },
				{ id: "cargo-test", name: "cargo test", icon: "terminal" },
			],
		},
	],
};

/** A shell given nearly all the room, the branch's page squeezed to its glyph. */
const narrow: Tiling = {
	split: "across",
	sizes: [1, 9],
	parts: [
		{ id: "free-shipping", name: "free-shipping", icon: "branch", home: true },
		{ id: "shell", name: shell, icon: "terminal" },
	],
};

/** Two columns of two: the page and its tests, the agent and the changes it made. */
const four: Tiling = {
	split: "across",
	parts: [
		{
			split: "down",
			parts: [
				{ id: "sales-tax", name: "sales-tax", icon: "branch", home: true },
				{
					id: "cargo-test",
					name: "cargo test",
					icon: "terminal",
					status: status("passed", "Finished"),
				},
			],
		},
		{
			split: "down",
			parts: [
				{ id: "claude", name: "claude", icon: "terminal", status: waiting },
				{ id: "uncommitted", name: "Uncommitted changes", icon: "diff" },
			],
		},
	],
};

/** Waits for the rows of the split whose map is pointed at to light, a delay after the pointer. */
const lit = async (row: HTMLElement | null, want: boolean) => {
	for (let tries = 0; tries < 60; tries++) {
		if ((row?.closest("[data-linked]") != null) === want) return;
		await new Promise((resolve) => setTimeout(resolve, 25));
	}
	throw new Error(`The row ${want ? "never lit" : "stayed lit"}`);
};

/**
 * A worktree as the sidebar lists it, two of its branches split: sales-tax beside claude and its
 * tests, free-shipping beside a shell. Each row a split shows carries its map, its own tile inked;
 * pointing at any of them opens the preview and lights the other rows of that split. A press on a
 * row selects it, as opening it would.
 */
const Worktree: FC = () => {
	const [selected, setSelected] = useState("docs-typo");
	const [pointed, setPointed] = useState<Tiling | null>(null);
	const splits = [three, narrow];
	const splitOf = (id: string) => splits.find((split) => tileIds(split).includes(id));
	const map = (id: string) => {
		const split = splitOf(id);
		if (split === undefined) return undefined;
		return (
			<SplitPreviewMap
				tiling={split}
				here={id}
				onOpenChange={(open) => setPointed((was) => (open ? split : was === split ? null : was))}
			/>
		);
	};
	const row = (id: string) => ({
		selected: selected === id,
		linked: pointed !== null && tileIds(pointed).includes(id),
		split: map(id),
		onClick: () => setSelected(id),
	});
	return (
		<Card header={machine} label="pavel--macbook-pro-2">
			<Repository last>
				<RepoItem name="shop" folded={false} />
				<WorktreeItem
					name="Main worktree"
					main
					depth={1}
					folded={false}
					meta={<MetaCount type="age">3d</MetaCount>}
				/>
				<UncommittedItem count={12} depth={2} {...row("uncommitted")} />
				<TerminalItem name="claude" depth={2} {...row("claude")} meta={waiting} />
				<TerminalItem name="cargo test" depth={2} {...row("cargo-test")} />
				<TerminalItem name={shell} depth={2} {...row("shell")} />
				<BranchItem
					title="free-shipping"
					depth={2}
					folded
					{...row("free-shipping")}
					meta={<MetaCount type="unpushed">1</MetaCount>}
				/>
				<BranchItem
					title="sales-tax"
					depth={2}
					folded
					{...row("sales-tax")}
					meta={<MetaCount type="unpushed">2</MetaCount>}
				/>
				<BranchItem
					title="docs-typo"
					depth={2}
					folded
					{...row("docs-typo")}
					meta={<MetaCount type="unpushed">1</MetaCount>}
				/>
			</Repository>
		</Card>
	);
};

const meta = stories({
	title: "Workspace tree/Splits",
	id: "components-splitpreview",
	component: SplitPreview,
	args: { tiling: three },
});

/** What a row's split holds, as the map on its row opens it: each tile by the row it shows. */
export const Default = meta.story({
	parameters: { design: { type: "figma", url: butUi("5431-221396") } },
});

/** Opened from claude's map: its row marked in the list. */
export const FromARow = meta.story({
	args: { tiling: three, here: "claude" },
});

/**
 * The layouts as they were left, each named after the row it was made on: the map draws the tiles'
 * shares of the room, the list names every row in full however small its tile.
 */
export const Layouts = meta.story({
	render: () => (
		<div style={{ display: "flex", flexWrap: "wrap", gap: 24, alignItems: "flex-start" }}>
			<SplitPreview tiling={three} />
			<SplitPreview tiling={narrow} />
			<SplitPreview tiling={four} />
		</div>
	),
});

/** The map on its own: the same split as each of its rows marks it, that row's tile inked. Point
 * at one to open its preview. */
export const Map = meta.story({
	parameters: { design: { type: "figma", url: butUi("5417-220251") } },
	render: () => (
		<div style={{ display: "flex", gap: 16 }}>
			{tileIds(four).map((id) => (
				<SplitPreviewMap key={id} tiling={four} here={id} />
			))}
		</div>
	),
});

/**
 * You're on docs-typo. Two branches were split: each row a split shows carries its map, its own
 * tile inked. Point at any map: its preview opens and the other rows of that split light up. Press
 * a row to select it.
 */
export const InARepository = meta.story({
	parameters: {
		layout: "fullscreen",
		height: 420,
		design: { type: "figma", url: butUi("5431-221416") },
	},
	play: async ({ canvas, userEvent }) => {
		const claude = canvas.getByText("claude");
		const [map] = canvas.getAllByRole("img", { name: /^Split: sales-tax/ });
		if (map === undefined) throw new Error("sales-tax's split has no map");
		await userEvent.hover(map);
		await lit(claude, true);
		await lit(canvas.getByText(shell), false);
	},
	render: () => (
		<div style={{ padding: 24 }}>
			<div data-split-preview-edge data-split-room style={{ width: "fit-content" }}>
				<Worktree />
			</div>
		</div>
	),
});
