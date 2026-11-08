import { stories, type Decorator } from "../docs/stories.ts";
import { Button } from "./Button.tsx";
import { Icon } from "./Icon.tsx";
import {
	StatusAlert,
	StatusBar,
	StatusHint,
	StatusLocation,
	StatusNote,
	StatusOperation,
	StatusText,
} from "./StatusBar.tsx";

const core = (node: string) => ({
	design: {
		type: "figma",
		url: `https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=${node}`,
	},
});

const meta = stories({
	title: "Preview/StatusBar",
	id: "components-statusbar",
	component: StatusBar,
	parameters: { ...core("4852-2389"), layout: "bar" },
});

const pane: Decorator = (Story) => (
	<div style={{ display: "flex", flexDirection: "column", alignItems: "start", gap: 12 }}>
		<Story />
	</div>
);

const Conflicts = () => (
	<StatusAlert>
		<StatusNote tone="danger" icon="commit">
			3 commits
		</StatusNote>
		<StatusNote tone="danger" icon="diff">
			2 uncommitted files
		</StatusNote>
	</StatusAlert>
);

const Commands = () => <StatusNote icon="keyboard">⌘K Commands</StatusNote>;

const hints = (
	<>
		<StatusHint hotkey="R">Rename</StatusHint>
		<Commands />
	</>
);

const branchHints = (
	<>
		<StatusHint hotkey="Mod+B">Pull Request</StatusHint>
		<StatusHint hotkey="R">Rename</StatusHint>
		<StatusHint hotkey="Mod+R">Fetch</StatusHint>
		<Commands />
	</>
);

const UndoUpdate = () => <StatusHint hotkey="Mod+Z">Undo update from upstream</StatusHint>;

const FetchedJustNow = () => <StatusNote iconEnd="refresh">Fetched just now</StatusNote>;

/** A branch selected: how fresh the remote is and where edits land, then the keys for a branch. */
export const Default = meta.story({
	render: () => (
		<StatusBar
			start={
				<>
					<StatusNote iconEnd="refresh">Fetched 2m ago</StatusNote>
					<StatusNote icon="commit">1 unread</StatusNote>
					<StatusLocation place="Main worktree" />
				</>
			}
			end={hints}
		/>
	),
});

/** Conflicts come right after the fetch, behind their badge; only the places that have some. */
export const ConflictedCommits = meta.story({
	parameters: core("4859-2823"),
	render: () => (
		<StatusBar
			start={
				<>
					<FetchedJustNow />
					<StatusAlert>
						<StatusNote tone="danger" icon="commit">
							2 commits
						</StatusNote>
					</StatusAlert>
					<StatusLocation place="Main worktree" />
				</>
			}
			end={
				<>
					<UndoUpdate />
					<Commands />
				</>
			}
		/>
	),
});

/** Uncommitted files in conflict, in a linked worktree: the location names it. */
export const FilesInLinkedWorktree = meta.story({
	parameters: core("4859-3437"),
	render: () => (
		<StatusBar
			start={
				<>
					<StatusNote iconEnd="refresh">Fetched 2m ago</StatusNote>
					<StatusAlert>
						<StatusNote tone="danger" icon="diff">
							3 uncommitted files
						</StatusNote>
					</StatusAlert>
					<StatusLocation place="feature-x" />
				</>
			}
			end={branchHints}
		/>
	),
});

/** A note at rest is quiet text/2; `danger` is for something broken and the conflicts' counts. */
export const Notes = meta.story({
	parameters: { ...core("4852-2344"), layout: undefined },
	decorators: [pane],
	render: () => (
		<>
			<StatusNote iconEnd="refresh">Fetched 2m ago</StatusNote>
			<StatusNote icon="commit">1 unread</StatusNote>
			<StatusNote tone="danger" icon="cross">
				CI failed
			</StatusNote>
			<StatusNote tone="danger" icon="warning">
				Couldn’t fetch
			</StatusNote>
		</>
	),
});

/** The conflicts behind one badge: a note per place, each opening the first conflict there. */
export const ConflictsAlert = meta.story({
	parameters: { ...core("4858-2610"), layout: undefined },
	render: () => <Conflicts />,
});

/** A failed fetch takes the place of "Fetched 2m ago", in the danger tone; a click tries again. */
export const FetchFailed = meta.story({
	parameters: core("4859-3298"),
	render: () => (
		<StatusBar
			start={
				<>
					<StatusNote tone="danger" iconEnd="refresh">
						Couldn’t fetch
					</StatusNote>
					<StatusLocation place="Main worktree" />
				</>
			}
			end={branchHints}
		/>
	),
});

/** Stepping through a diff's files: where you are, as plain text after the notes. */
export const GoingThroughFiles = meta.story({
	parameters: core("4883-12649"),
	render: () => (
		<StatusBar
			start={
				<>
					<FetchedJustNow />
					<StatusText>File 12 of 36</StatusText>
				</>
			}
			end={
				<>
					<UndoUpdate />
					<Commands />
				</>
			}
		/>
	),
});

/** A dev build offers its own action last in the end: one small outline button. */
export const DevButton = meta.story({
	parameters: core("4883-13191"),
	render: () => (
		<StatusBar
			start={<FetchedJustNow />}
			end={
				<>
					<UndoUpdate />
					<Commands />
					<Button size="small" variant="outline">
						Build and restart
						<Icon name="play" />
					</Button>
				</>
			}
		/>
	),
});

/** While a commit is being moved the start says what goes where, and the end holds the move's keys. */
export const Moving = meta.story({
	parameters: core("4859-3091"),
	render: () => (
		<StatusBar
			start={
				<StatusOperation
					op="move"
					subjectIcon="commit"
					subject="Add test file 18 to collection"
					where="to the top of"
					placeIcon="branch"
					place="pl-branch-3"
				/>
			}
			end={
				<>
					<StatusHint hotkey="Enter">Drop here</StatusHint>
					<StatusHint hotkey="Escape">Cancel</StatusHint>
				</>
			}
		/>
	),
});

/** Asked for and under way: the clock, every word in text/2, and the time taken once it's noticeable. */
export const Running = meta.story({
	render: () => (
		<StatusBar
			start={
				<StatusOperation
					op="move"
					running
					where="below"
					placeIcon="commit"
					place="d6f0501"
					elapsed="3s"
				/>
			}
			end={<Commands />}
		/>
	),
});

/** Each operation as the bar says it. Commit and amend have no subject: the composer shows it. */
export const Operations = meta.story({
	parameters: { ...core("4863-2883"), layout: undefined },
	decorators: [pane],
	render: () => (
		<>
			<StatusOperation
				op="move"
				subjectIcon="commit"
				subject="Add test file 18 to collection"
				where="to the top of"
				placeIcon="branch"
				place="pl-branch-3"
			/>
			<StatusOperation
				op="squash"
				subjectIcon="commit"
				subject="Fix typo in test file creation message"
				where="into"
				placeIcon="commit"
				place="Add test file 20 and update content"
			/>
			<StatusOperation
				op="amend"
				where="into"
				placeIcon="commit"
				place="Fix typo in the error message"
			/>
			<StatusOperation op="commit" where="to the top of" placeIcon="branch" place="feature-x" />
			<StatusOperation
				op="cherry-pick"
				subjectIcon="commit"
				subject="Fix typo in the error message"
				where="below"
				placeIcon="commit"
				place="Validate the email field"
			/>
			<StatusOperation op="cut" subjectIcon="commit" subject="Fix typo in the error message" />
			<StatusOperation op="move" subjectIcon="commit" subject="3 commits" where="to a new stack" />
		</>
	),
});

/** Where edits land, by the worktree's name. */
export const Location = meta.story({
	parameters: { ...core("4858-2670"), layout: undefined },
	render: () => <StatusLocation place="Main worktree" />,
});

/** Plain words that aren't a way in: where you are as you step through a diff's files. */
export const Text = meta.story({
	parameters: { ...core("4892-2886"), layout: undefined },
	render: () => <StatusText>File 12 of 36</StatusText>,
});

/** A key and what it does; undo names the last action. */
export const Hints = meta.story({
	parameters: { ...core("4852-2346"), layout: undefined },
	decorators: [pane],
	render: () => (
		<>
			<StatusHint hotkey="Mod+Z">Undo move commit</StatusHint>
			<StatusHint hotkey="R">Rename</StatusHint>
		</>
	),
});

/** In a narrow window, here 480px, the notes that don't fit drop whole from the end: the conflicts and everything after them. */
export const Narrow = meta.story({
	parameters: { width: 480 },
	render: () => (
		<StatusBar
			start={
				<>
					<StatusNote iconEnd="refresh">Fetched 2m ago</StatusNote>
					<Conflicts />
					<StatusNote icon="commit">1 unread</StatusNote>
				</>
			}
			end={hints}
		/>
	),
});

/** An operation doesn't drop in a narrow window: its subject and place are ellipsed instead. */
export const NarrowOperation = meta.story({
	parameters: { width: 480 },
	render: () => (
		<StatusBar
			start={
				<StatusOperation
					op="squash"
					subjectIcon="commit"
					subject="Fix typo in test file creation message"
					where="into"
					placeIcon="commit"
					place="Add test file 20 and update content"
				/>
			}
			end={hints}
		/>
	),
});
