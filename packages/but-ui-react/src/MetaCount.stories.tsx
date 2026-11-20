import { stories } from "../stories/stories.ts";
import { Button } from "./Button.tsx";
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
					5
					<Icon name="arrow-up" />
				</Button>
			</Tooltip>
		</div>
	),
});
