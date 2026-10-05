import preview from "#storybook/preview";
import { Toggle, ToggleGroup } from "@base-ui/react";
import { Avatar } from "./Avatar.tsx";
import { Badge } from "./Badge.tsx";
import { Button } from "./Button.tsx";
import { CopyableId } from "./CopyableId.tsx";
import { DropdownButton } from "./DropdownButton.tsx";
import { EntityAvatar } from "./EntityAvatar.tsx";
import { machinePictures } from "./story-assets/machines.ts";
import { Icon } from "./Icon.tsx";
import { SwitchButton } from "./SwitchButton.tsx";
import { ToggleGroupStyles, ToggleStyles } from "./ToggleGroup.tsx";
import { ViewHeader, ViewHeaderDivider } from "./ViewHeader.tsx";
import type { FC } from "react";

const meta = preview.meta({
	component: ViewHeader,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/EBuHQGUcCaSw4Ln5uVpWkn/Client?node-id=5611-127048",
		},
	},
	decorators: [
		// The details pane the header tops, at the width it usually has.
		(Story) => (
			<div style={{ width: 820, backgroundColor: "var(--bg-1)" }}>
				<Story />
			</div>
		),
	],
});

const BranchTabs: FC<{ tab?: "diff" | "pr"; prDisabled?: boolean }> = ({
	tab = "diff",
	prDisabled = false,
}) => (
	<ToggleGroup render={<ToggleGroupStyles />} defaultValue={[tab]} aria-label="Branch tab">
		<Toggle render={<ToggleStyles />} value="diff">
			Diff
		</Toggle>
		<Toggle render={<ToggleStyles />} value="pr" disabled={prDisabled}>
			{prDisabled ? "No pull request" : "Pull Request"}
		</Toggle>
	</ToggleGroup>
);

const CommitFilter: FC = () => (
	<Button>
		<Icon name="commit" />
		All 4 commits
	</Button>
);

const copyToClipboard = (value: string) => void navigator.clipboard.writeText(value);

/** The author line every commit view shares: who, when, and the two IDs, each copied on click. */
const CommitAuthorLine: FC = () => (
	<>
		<Avatar src={null} seed="pavel@gitbutler.com" alt="Commit author avatar" />
		<span>Pavel Laptev at 12.09.2026, 18:34</span>
		<CopyableId
			label="Copy change ID"
			icon="finger-print"
			value="kzqtpwoylnsrkrkmxqzvwtuoyrpslmnk"
			display="kzqtpwo"
			onCopy={copyToClipboard}
		/>
		<CopyableId
			label="Copy commit ID"
			icon="hash"
			value="1cd28ec4f0a9b3e1d2c7a6f5e4b3a2918d7c6b5a"
			display="1cd28ec"
			onCopy={copyToClipboard}
		/>
	</>
);

/** A commit's message body, as the toggle after its title unfolds it. */
const CommitBody: FC = () => (
	<p className="text-monospace text-body" style={{ fontSize: 12, whiteSpace: "pre-wrap" }}>
		{`Search took the query as typed, so a stray quote or a backslash broke the
index lookup. The query is now escaped before it reaches the index, and an
empty one returns nothing instead of every document.`}
	</p>
);

const SendAndMenu: FC = () => (
	<>
		<Button>
			Send
			<Icon name="arrow-right" />
		</Button>
		<Button iconOnly aria-label="More">
			<Icon name="kebab" />
		</Button>
	</>
);

const branchMeta =
	"4 commits ahead of origin/master · 36 behind · draft #15941 · 2 of 32 checks failing";

export const Default = meta.story({
	args: {
		icon: "branch",
		title: "feature/JIRA-404-search-filters",
		meta: branchMeta,
	},
});

/** An applied branch on the Diff tab: its tabs, a divider and the commit filter. No buttons. */
export const Branch = meta.story({
	args: {
		icon: "branch",
		title: "feature/JIRA-404-search-filters",
		meta: branchMeta,
		toolbar: (
			<>
				<BranchTabs />
				<ViewHeaderDivider />
				<CommitFilter />
			</>
		),
	},
});

/**
 * Lite in full-window mode: the window's controls lead the title's line, after the room the
 * traffic lights take. The meta line and the toolbar still start at the header's edge.
 */
export const BranchFullWindow = meta.story({
	args: {
		leading: (
			<div style={{ display: "flex", columnGap: 12, alignItems: "center" }}>
				<div style={{ width: 64 }} />
				<Button iconOnly variant="ghost" aria-label="Show the sidebar">
					<Icon name="sidebar-narrow" />
				</Button>
				<ViewHeaderDivider />
			</div>
		),
		icon: "branch",
		title: "feature/JIRA-404-search-filters",
		meta: branchMeta,
		toolbar: (
			<>
				<BranchTabs />
				<ViewHeaderDivider />
				<CommitFilter />
			</>
		),
	},
});

/**
 * The Pull Request tab: the commit filter leaves with the diff, and the pull request's own
 * toolbar takes the end. Ghosts for Edit and the menu, an outline switch for Auto-merge since it
 * holds state, and Merge as the surface's one pop.
 */
export const BranchPullRequestTab = meta.story({
	args: {
		icon: "branch",
		title: "feature/JIRA-404-search-filters",
		meta: branchMeta,
		toolbar: <BranchTabs tab="pr" />,
		actions: (
			<>
				<Button variant="ghost">
					Edit
					<Icon name="edit" />
				</Button>
				<SwitchButton label="Auto-merge" variant="outline" />
				<DropdownButton variant="pop" menuLabel="Merge options" onMenuTrigger={() => {}}>
					Merge
				</DropdownButton>
				<Button variant="ghost" iconOnly aria-label="Pull request menu">
					<Icon name="kebab" />
				</Button>
			</>
		),
	},
});

/** A branch outside the workspace: no review, so the tab says why, and Apply is its action. */
export const BranchNotApplied = meta.story({
	args: {
		icon: "branch",
		title: "debug-this-nightmare",
		meta: "3 commits ahead of origin/master",
		toolbar: <BranchTabs prDisabled />,
		actions: <Button variant="gray">Apply to workspace</Button>,
	},
});

/** No tabs, so no toolbar: the author line ends the header. */
export const Commit = meta.story({
	args: {
		icon: "commit",
		title: "fix: sanitize user input in search query",
		meta: <CommitAuthorLine />,
		children: <CommitBody />,
	},
});

/** A commit whose review has landed: its tabs lead the author line rather than take a row. */
export const CommitLandedReview = meta.story({
	args: {
		icon: "commit",
		title: "Add test file 18 to collection",
		metaTabs: <BranchTabs tab="pr" />,
		meta: <CommitAuthorLine />,
		children: <CommitBody />,
	},
});

/** A conflicted commit: the badge keeps its size while the subject gives way. */
export const CommitConflicted = meta.story({
	args: {
		icon: "commit",
		title: (
			<>
				<span>fix: sanitize user input in search query before it reaches the index</span>
				<Badge variant="danger">Conflicted</Badge>
			</>
		),
		meta: <CommitAuthorLine />,
		children: <CommitBody />,
	},
});

/**
 * but.dev's commit, stepping through a branch's commits. No toolbar, so the buttons sit level
 * with the author line.
 */
export const CommitWithPicker = meta.story({
	args: {
		icon: "commit",
		title: "fix: sanitize user input in search query",
		meta: <CommitAuthorLine />,
		children: <CommitBody />,
		actions: (
			<>
				<Button iconOnly aria-label="Previous commit">
					<Icon name="arrow-left" />
				</Button>
				<Button>1 of 4</Button>
				<Button iconOnly aria-label="Next commit">
					<Icon name="arrow-right" />
				</Button>
			</>
		),
	},
});

/** A clean worktree: the title alone. */
export const Uncommitted = meta.story({
	args: {
		icon: "file-diff",
		title: "Uncommitted",
	},
});

/** but.dev's repository checkout: what it holds besides commits, and where to send it. */
export const Worktree = meta.story({
	args: {
		icon: "folder",
		title: "gitbutler",
		meta: "Workspace · 2 branches · 4 unpushed · 3 uncommitted files",
		actions: <SendAndMenu />,
	},
});

/** but.dev's machine: an entity, so it wears the picture of its kind as its avatar. */
export const Machine = meta.story({
	args: {
		avatar: <EntityAvatar value={{ _tag: "Picture", src: machinePictures[0].src }} />,
		title: "pave--macbook-pro",
		meta: "macOS · but 0.19 · connected since 3 Sep",
		actions: <SendAndMenu />,
	},
});

/** but.dev's cloud session: an avatar, the branch's tabs, and Send to… as the surface's pop. */
export const CloudSession = meta.story({
	args: {
		avatar: <EntityAvatar value={{ _tag: "Emoji", emoji: "☁️" }} />,
		title: "Panel incoming commits count",
		meta: "Claude Code · started from your phone · works on feature/JIRA-404-search-filters · 4 commits · 3 uncommitted",
		toolbar: (
			<>
				<BranchTabs />
				<ViewHeaderDivider />
				<CommitFilter />
			</>
		),
		actions: (
			<Button variant="pop">
				<Icon name="arrow-right" />
				Send to…
			</Button>
		),
	},
});
