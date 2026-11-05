import { stories, type Decorator } from "../docs/stories.ts";
import {
	StatusAlert,
	StatusBar,
	StatusHint,
	StatusLocation,
	StatusNote,
	StatusOperation,
} from "./StatusBar.tsx";

const core = (node: string) => ({
	design: {
		type: "figma",
		url: `https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=${node}`,
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

/** Conflicts come right after the fetch, behind their badge, then what else is broken. */
export const WithConflicts = meta.story({
	render: () => (
		<StatusBar
			start={
				<>
					<StatusNote iconEnd="refresh">Fetched 2m ago</StatusNote>
					<Conflicts />
					<StatusNote tone="danger" icon="cross">
						CI failed
					</StatusNote>
				</>
			}
			end={<Commands />}
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

/** A failed fetch takes the place of "Fetched 2m ago", in the danger tone. */
export const FetchFailed = meta.story({
	render: () => (
		<StatusBar
			start={
				<>
					<StatusNote tone="danger" icon="warning">
						Couldn’t fetch
					</StatusNote>
					<StatusNote icon="commit">1 unread</StatusNote>
					<StatusLocation place="feature-x" />
				</>
			}
			end={hints}
		/>
	),
});

/** While a commit is being moved the start says what goes where, and the end holds the move's keys. */
export const Moving = meta.story({
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
					<StatusHint hotkey="Enter">Move</StatusHint>
					<StatusHint hotkey="Escape">Cancel</StatusHint>
					<Commands />
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
