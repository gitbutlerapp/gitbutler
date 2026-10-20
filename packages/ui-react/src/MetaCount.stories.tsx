import preview from "#storybook/preview";
import { CiStatus, MetaCount, type MetaCountType } from "./MetaCount.tsx";

const types: Array<MetaCountType> = [
	"behind",
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

const meta = preview.meta({
	title: "Sidebar/MetaCount",
	id: "components-metacount",
	component: MetaCount,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=2572-3286",
		},
	},
	argTypes: {
		type: { control: "select", options: types },
	},
	args: { type: "unpushed", children: "4" },
});

/** One count: its glyph, coloured by what it counts, and its value. */
export const Default = meta.story({ args: { type: "unpushed", children: "4" } });

const sample: Record<MetaCountType, string> = {
	behind: "3",
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

/** Every kind of count, as rows show them, and the three states of a branch's checks. */
export const Kinds = meta.story({
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=2572-3595",
		},
	},
	render: () => (
		<div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
			<div style={{ display: "flex", flexWrap: "wrap", gap: 12 }}>
				{types.map((type) => (
					<MetaCount key={type} type={type}>
						{sample[type]}
					</MetaCount>
				))}
			</div>
			<div style={{ display: "flex", gap: 12 }}>
				<CiStatus status="working" />
				<CiStatus status="passed" />
				<CiStatus status="failed" />
			</div>
		</div>
	),
});

/** Checks with how far they got, as a pull request's tab shows them: 2 of 14 done, or failed. */
export const CiProgress = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 12 }}>
			<CiStatus status="working">CI 2/14</CiStatus>
			<CiStatus status="failed">CI 2/14</CiStatus>
			<CiStatus status="passed" />
		</div>
	),
});
