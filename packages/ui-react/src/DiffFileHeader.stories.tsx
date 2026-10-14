import preview from "#storybook/preview";
import { useState } from "react";
import { Badge } from "./Badge.tsx";
import { DiffFileHeader, type DiffFileReviewState } from "./DiffFileHeader.tsx";

const meta = preview.meta({
	title: "Content & status/DiffFileHeader",
	id: "components-difffileheader",
	component: DiffFileHeader,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=2415-6189",
		},
	},
	argTypes: {
		reviewState: { control: "inline-radio", options: ["unreviewed", "reviewed", "changed"] },
	},
	args: {
		path: "src/components/Button.svelte",
		added: 34,
		removed: 28,
		collapsed: false,
		reviewState: "unreviewed" as const,
		onCollapsedChange: () => {},
		onReviewedChange: () => {},
		onMenu: () => {},
	},
	decorators: [
		(Story) => (
			<div style={{ width: 546 }}>
				<Story />
			</div>
		),
	],
});

export const Default = meta.story({});

/** Reviewed, then changed since: the box goes mixed until the reader looks again. */
export const Review = meta.story({
	render: function Render(args) {
		const [reviewState, setReviewState] = useState<DiffFileReviewState>("changed");

		return (
			<DiffFileHeader
				{...args}
				reviewState={reviewState}
				onReviewedChange={(reviewed) => setReviewState(reviewed ? "reviewed" : "unreviewed")}
			/>
		);
	},
});

/** A diff shown for reading only, as a pull request's comment anchors it: the file and its counts. */
export const ReadOnly = meta.story({
	args: {
		onCollapsedChange: undefined,
		onReviewedChange: undefined,
		onMenu: undefined,
	},
});

/** Something the file needs said goes before its actions. */
export const Conflicted = meta.story({
	args: {
		path: "crates/but-workspace/src/commit.rs",
		added: undefined,
		removed: undefined,
		onReviewedChange: undefined,
		onMenu: undefined,
		children: <Badge variant="danger">Conflicted</Badge>,
	},
});

/** A deep path gives way before the name does. */
export const LongPath = meta.story({
	args: {
		path: "apps/lite/ui/src/routes/project/$id/workspace/WorkspaceLists/UncommittedChangesRow.tsx",
		added: 1204,
		removed: 3,
	},
	decorators: [
		(Story) => (
			<div style={{ width: 420 }}>
				<Story />
			</div>
		),
	],
});
