import { stories } from "../docs/stories.ts";
import { useState, type FC } from "react";
import { CommitItem, NoticeItem, TargetItem, WorktreeItem } from "./SidebarRow.tsx";
import { Column, update } from "./story-assets/workspaceTree.tsx";

const core = (node: string) =>
	`https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=${node}`;

const meta = stories({
	title: "Workspace tree/Target",
	id: "components-sidebarrow-target",
	component: TargetItem,
	parameters: { width: 316 },
	args: { target: "master", count: 4, depth: 1, folded: true },
});

/** What the target has that the workspace doesn't: how many commits, with Update beside them. */
export const Default = meta.story({
	render: (args) => <TargetItem {...args} actions={update} />,
});

/**
 * What the target has that the workspace doesn't, over the repository's worktrees: how many
 * commits, with Update beside them and who they come from before it. Open, its commits sit under
 * it, each with its author's picture; a merge opens to the commits it brought in.
 */
export const NewOnTarget = meta.story({
	parameters: { design: { type: "figma", url: core("4664-11729") } },
	render: () => <NewOnTargetTree />,
});

/**
 * A {@link TargetItem} whose commits couldn't load: a NoticeItem says so in their place, with the
 * reason over it and Try again after it.
 */
export const NewOnTargetFailed = meta.story({
	parameters: { design: { type: "figma", url: core("4318-12241") } },
	render: () => (
		<Column>
			<TargetItem target="master" count={2} depth={1} folded={false} actions={update} />
			<NoticeItem
				message="Can’t show the commits."
				tooltip="The machine didn’t answer"
				action={{ label: "Try again", onClick: () => {} }}
				depth={2}
			/>
			<WorktreeItem name="Main worktree" main depth={1} folded />
		</Column>
	),
});

const pavel = { seed: "pavel@gitbutler.com" };
const kiril = { seed: "kiril@gitbutler.com" };

/** {@link NewOnTarget}'s rows, folding as the sidebar's do. */
const NewOnTargetTree: FC = () => {
	const [open, setOpen] = useState(true);
	const [merge, setMerge] = useState(true);
	return (
		<div>
			<TargetItem
				target="master"
				count={74}
				authors={[pavel, kiril]}
				tooltip="74 new commits on master, from Pavel Laptev and Kiril Videlov"
				depth={1}
				folded={!open}
				onToggleFolded={() => setOpen(!open)}
				actions={update}
			/>
			{open && (
				<>
					<CommitItem
						message="chore: separator tokens"
						merge
						tooltip="Pavel Laptev · 2h ago"
						depth={2}
						folded={!merge}
						onToggleFolded={() => setMerge(!merge)}
					/>
					{merge && (
						<>
							<CommitItem
								message="chore: separator tokens"
								author={pavel}
								tooltip="Pavel Laptev · 2h ago"
								depth={3}
							/>
							<CommitItem
								message="feature: user authentication"
								author={pavel}
								tooltip="Pavel Laptev · 3h ago"
								depth={3}
							/>
						</>
					)}
					<CommitItem
						message="fix: retry stalled transfers"
						author={kiril}
						tooltip="Kiril Videlov · 5h ago"
						depth={2}
					/>
				</>
			)}
			<WorktreeItem name="Main worktree" main depth={1} folded />
		</div>
	);
};
