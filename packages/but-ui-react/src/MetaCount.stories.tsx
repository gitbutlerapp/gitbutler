import type { FC, ReactNode } from "react";
import { stories } from "../stories/stories.ts";
import { Button } from "./Button.tsx";
import { Code } from "./Code.tsx";
import { DescriptionList, DescriptionListItem } from "./DescriptionList.tsx";
import { Icon } from "./Icon.tsx";
import { MetaCount, type MetaCountType, MetaStatus } from "./MetaCount.tsx";
import { Tooltip } from "./Tooltip.tsx";

const types: Array<MetaCountType> = [
	"behind",
	"conflicts",
	"unpushed",
	"rebased",
	"commits",
	"empty",
	"head",
	"uncommitted",
	"clean",
	"age",
	"branch",
	"worktrees",
	"repos",
	"agent-working",
	"agent-waiting",
	"running",
	"stack",
	"cpu",
	"cpu-danger",
];

const meta = stories({
	title: "Content & status/MetaCount",
	id: "components-metacount",
	component: MetaCount,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=2572-3286",
		},
	},
	args: { type: "unpushed", children: "4" },
});

/** One count: its glyph, coloured by what it counts, and its value. */
export const Default = meta.story({ args: { type: "unpushed", children: "4" } });

const sample: Record<MetaCountType, string> = {
	behind: "3",
	conflicts: "2",
	unpushed: "4",
	rebased: "2",
	commits: "5",
	empty: "empty",
	head: "detached",
	uncommitted: "12",
	clean: "0",
	age: "3d",
	branch: "retry-transfers",
	worktrees: "2",
	repos: "4",
	"agent-working": "12m",
	"agent-waiting": "2m",
	running: "cargo build",
	stack: "1/3",
	cpu: "14%",
	"cpu-danger": "84%",
};

/** Every kind of count, as rows show them. */
export const Kinds = meta.story({
	render: () => (
		<div style={{ display: "flex", flexWrap: "wrap", gap: 12 }}>
			{types.map((type) => (
				<MetaCount key={type} type={type}>
					{sample[type]}
				</MetaCount>
			))}
		</div>
	),
});

/** A mark that says something is running, and the code that draws it. */
const Mark: FC<{ children: ReactNode; code: string }> = ({ children, code }) => (
	<span style={{ display: "inline-flex", alignItems: "center", gap: 8 }}>
		{children}
		<Code>{code}</Code>
	</span>
);

/**
 * Every mark that says something is running, side by side, picked by who is at work: the ring for
 * your own action, in the row's glyph or a loading snackbar; the amber ring for a check or a deploy,
 * a result you'll look at; the robot for an agent; the dots for any other program in a terminal.
 * Only the amber one asks anything of you. The Work under way note has the rules.
 */
export const WorkUnderWay = meta.story({
	render: () => (
		<DescriptionList>
			<DescriptionListItem label="Your action">
				<Mark code="busy">
					<Icon name="spinner" aria-label="Working" />
				</Mark>
			</DescriptionListItem>
			<DescriptionListItem label="Out of sight">
				<Mark code={'type: "loading"'}>
					<Icon name="spinner" aria-label="Working" />
				</Mark>
			</DescriptionListItem>
			<DescriptionListItem label="A check">
				<Mark code={'<MetaStatus variant="working">'}>
					<MetaStatus variant="working" aria-label="Checks running" />
				</Mark>
			</DescriptionListItem>
			<DescriptionListItem label="An agent">
				<Mark code={'<MetaCount type="agent-working">'}>
					{/* oxlint-disable-next-line jsx_a11y/prefer-tag-over-role -- A drawn glyph with a name, as MetaStatus alone is. */}
					<MetaCount type="agent-working" role="img" aria-label="Agent working" />
				</Mark>
			</DescriptionListItem>
			<DescriptionListItem label="A program">
				<Mark code={'<MetaCount type="running">'}>
					{/* oxlint-disable-next-line jsx_a11y/prefer-tag-over-role -- A drawn glyph with a name, as MetaStatus alone is. */}
					<MetaCount type="running" role="img" aria-label="Working" />
				</Mark>
			</DescriptionListItem>
		</DescriptionList>
	),
});

/**
 * A count with a next step, as a branch row ends: a count isn't pressed, so the step is a small
 * `Button` in its place, in the count's colour (`info` for unpushed, `warning` for behind): the
 * number and then the count's glyph, 4px after the counts before it. Its tooltip says what pressing
 * it does.
 */
export const WithNextStep = meta.story({
	render: () => (
		<div style={{ display: "flex", columnGap: 4, alignItems: "center" }}>
			<MetaStatus variant="working">CI</MetaStatus>
			<Tooltip content="Push 5 commits">
				<Button size="small" variant="info" aria-label="Push 5 commits">
					<Icon name="arrow-up" />5
				</Button>
			</Tooltip>
		</div>
	),
});
