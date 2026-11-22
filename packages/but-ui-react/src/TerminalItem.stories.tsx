import { stories } from "../stories/stories.ts";
import { MetaCount, MetaStatus, type MetaStatusVariant } from "./MetaCount.tsx";
import {
	BranchItem,
	RepoItem,
	TerminalItem,
	UncommittedItem,
	WorktreeItem,
} from "./SidebarRow.tsx";
import { Card, Column, machine, Repository, TerminalGroup } from "./story-assets/workspaceTree.tsx";
import { Tooltip } from "./Tooltip.tsx";

const core = (node: string) =>
	`https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=${node}`;

const meta = stories({
	title: "Workspace tree/Terminals",
	id: "components-sidebarrow-terminal",
	component: TerminalItem,
	parameters: { width: 316, design: { type: "figma", url: core("5393-4396") } },
	args: { name: "review-cards", place: "gpui-row-buttons" },
});

/** What a terminal's program is at, as its glyph alone, saying it in words under the pointer. */
const status = (variant: MetaStatusVariant, tip: string) => (
	<Tooltip content={tip}>
		<MetaStatus variant={variant} aria-label={tip} />
	</Tooltip>
);

/** An agent's terminal: the robot alone, at work or waiting for you. */
const agent = (type: "agent-working" | "agent-waiting", tip: string) => (
	<Tooltip content={tip}>
		{/* oxlint-disable-next-line jsx_a11y/prefer-tag-over-role -- A drawn glyph with a name, as MetaStatus alone is. */}
		<MetaCount type={type} role="img" aria-label={tip} />
	</Tooltip>
);

const waiting = agent("agent-waiting", "Waiting for you");
const working = agent("agent-working", "Working");
const failed = status("failed", "Command not found");
const finished = status("passed", "Finished");
const progressing = (
	<Tooltip content="Working">
		{/* oxlint-disable-next-line jsx_a11y/prefer-tag-over-role -- A drawn glyph with a name, as MetaStatus alone is. */}
		<MetaCount type="running" role="img" aria-label="Working" />
	</Tooltip>
);

/** A terminal, idle: its name, and where it's open after it, quieter. */
export const Default = meta.story({});

/**
 * Each thing a terminal's program can be at, as the program reports it. An agent takes the robot,
 * at work or waiting for you. Any other program that reports takes the terminal's own dots spinner
 * while it shows progress, then a glyph for a finish or a failure. An idle shell shows nothing, and
 * so does a program that reports nothing while it runs, as most commands.
 */
export const Statuses = meta.story({
	render: () => (
		<Column>
			<TerminalItem name="claude" place="gpui-row-buttons" meta={waiting} />
			<TerminalItem name="pi" place="review-cards" meta={working} />
			<TerminalItem name="cargo build" place="Main worktree" meta={progressing} />
			<TerminalItem name="cargo test" place="Main worktree" meta={finished} />
			<TerminalItem name="codex" place="Main worktree" meta={failed} />
			<TerminalItem name="Terminal" place="Main worktree" />
		</Column>
	),
});

/**
 * Terminals in their repository's card, under the worktree they're open on, after its branches.
 * The worktree above says where each is open, so they leave `place` out.
 */
export const InARepository = meta.story({
	parameters: { design: { type: "figma", url: core("5396-13018") } },
	render: () => (
		<Card header={machine} label="pavel--macbook-pro-2">
			<Repository last>
				<RepoItem name="gitbutler" folded={false} />
				<WorktreeItem
					name="Main worktree"
					main
					depth={1}
					folded={false}
					meta={<MetaCount type="age">5m</MetaCount>}
				/>
				<UncommittedItem count={3} depth={2} />
				<BranchItem
					title="gpui-terminal-command-names"
					depth={2}
					folded
					meta={<MetaCount type="unpushed">1</MetaCount>}
				/>
				<TerminalItem name="codex" depth={2} meta={failed} />
				<TerminalItem name="Terminal" depth={2} />
				<WorktreeItem
					name="gpui-row-buttons"
					depth={1}
					folded={false}
					meta={<MetaCount type="age">2h</MetaCount>}
				/>
				<BranchItem title="gpui-row-buttons" depth={2} folded />
				<TerminalItem name="review-cards" depth={2} meta={waiting} />
				<TerminalItem name="pi" depth={2} meta={working} />
			</Repository>
		</Card>
	),
});

/**
 * The same terminals folded away. A terminal's own row takes no dot, as its status is what you
 * haven't seen, so while a fold hides it the most folded row says it: the main worktree carries the
 * dot for `codex`, which failed since you last opened it, and the worktree with an agent waiting
 * shows its robot in its age's place. One with only an agent at work adds nothing.
 */
export const FoldedAway = meta.story({
	parameters: { design: { type: "figma", url: core("5396-13937") } },
	render: () => (
		<Card header={machine} label="pavel--macbook-pro-2">
			<Repository last>
				<RepoItem name="gitbutler" folded={false} />
				<WorktreeItem
					name="Main worktree"
					main
					depth={1}
					folded
					unread
					meta={<MetaCount type="age">5m</MetaCount>}
				/>
				<WorktreeItem name="gpui-row-buttons" depth={1} folded meta={waiting} />
				<WorktreeItem
					name="review-ios"
					depth={1}
					folded
					meta={<MetaCount type="age">40m</MetaCount>}
				/>
			</Repository>
		</Card>
	),
});

/**
 * Every terminal open, in the Terminals drawer at the sidebar's foot, under a label for each
 * status: what wants you first, what's under way last. Each says its repository and worktree,
 * as the drawer holds every project's.
 */
export const InTheTerminalsDrawer = meta.story({
	parameters: { design: { type: "figma", url: core("5396-14558") } },
	render: () => (
		<div
			style={{
				overflow: "hidden",
				border: "1px solid var(--but-border-subtle)",
				borderRadius: "var(--but-radius-card)",
				backgroundColor: "var(--but-background)",
			}}
		>
			<TerminalGroup
				label="Blocked"
				terminals={[{ name: "claude", place: "gitbutler / gpui-row-buttons", meta: waiting }]}
			/>
			<TerminalGroup
				label="Failed"
				terminals={[{ name: "codex", place: "gitbutler / Main worktree", meta: failed }]}
			/>
			<TerminalGroup
				label="Done"
				terminals={[{ name: "cargo build", place: "but-dev / Main worktree", meta: finished }]}
			/>
			<TerminalGroup
				label="Idle"
				terminals={[{ name: "Terminal", place: "test-repo-3 / Main worktree" }]}
			/>
			<TerminalGroup
				label="Cooking"
				terminals={[{ name: "pi", place: "but-dev / review-ios", meta: working }]}
				last
			/>
		</div>
	),
});

/**
 * The terminal showing, selected: the quiet fill while the keys are in the terminal, the solid one
 * while they're in the list. Its status stays where it is, inked for the solid fill.
 */
export const Selected = meta.story({
	render: () => (
		<div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
			<Column>
				<TerminalItem name="claude" place="gpui-row-buttons" meta={waiting} selected />
				<TerminalItem name="Terminal" place="Main worktree" />
			</Column>
			<div data-selection-focused="true">
				<Column>
					<TerminalItem name="claude" place="gpui-row-buttons" meta={waiting} selected />
					<TerminalItem name="Terminal" place="Main worktree" />
				</Column>
			</div>
		</div>
	),
});

/** A long name runs under the pointer; where it's open gives way first. */
export const LongNames = meta.story({
	render: () => (
		<Column>
			<TerminalItem
				name="claude --dangerously-skip-permissions"
				place="gitbutler / gpui-terminal-command-names"
				meta={waiting}
			/>
			<TerminalItem
				name="pavellaptev@Pavels-MacBook-Pro:~/Documents/GitHub/gitbutler"
				place="Main worktree"
			/>
		</Column>
	),
});
