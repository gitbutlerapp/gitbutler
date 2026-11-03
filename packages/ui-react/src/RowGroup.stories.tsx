import { stories } from "../docs/stories.ts";
import { useId, useState } from "react";
import { Badge } from "./Badge.tsx";
import { Cell } from "./Cell.tsx";
import { Icon } from "./Icon.tsx";
import { RowGroup } from "./RowGroup.tsx";
import { Section } from "./Section.tsx";
import { Switch } from "./Switch.tsx";

const figma = (nodeId: string) => ({
	design: {
		type: "figma",
		url: `https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=${nodeId}`,
	},
});

const meta = stories({
	title: "Content & status/RowGroup",
	id: "components-rowgroup",
	component: RowGroup,
	parameters: figma("4732-1025"),
	decorators: [
		(Story) => (
			<div style={{ maxWidth: 386 }}>
				<Story />
			</div>
		),
	],
});

export const Default = meta.story({
	args: {
		children: (
			<>
				<Cell icon="commit" title="Whole workspace" description="10 branches in 6 stacks" />
				<Cell icon="commit" title="128 uncommitted files" />
			</>
		),
	},
});

/** A setting that takes effect at once: the whole row is the switch's label. */
const Setting = ({ title, description }: { title: string; description?: string }) => {
	const id = useId();
	const [on, setOn] = useState(true);
	return (
		<Cell
			// oxlint-disable-next-line jsx_a11y/label-has-associated-control -- The row renders as this label, and Cell fills it with the title.
			render={<label htmlFor={id} />}
			title={title}
			description={description}
			trailing={<Switch id={id} size="large" checked={on} onCheckedChange={setOn} />}
		/>
	);
};

/** Settings under their title, each row a switch. */
export const Settings = meta.story({
	render: () => (
		<Section title="Follow-ups">
			<RowGroup>
				<Setting title="When CI fails" description="Starts an agent to fix the failing check." />
				<Setting title="When a review asks for changes" />
			</RowGroup>
		</Section>
	),
});

const sources = [
	{ id: "workspace", title: "Whole workspace", description: "10 branches in 6 stacks" },
	{ id: "uncommitted", title: "Uncommitted changes", description: "128 files" },
	{ id: "branch", title: "One branch", description: "You pick it next." },
];

/** One choice out of a few: each row is a button, and the chosen one has the tick. */
export const Picker = meta.story({
	render: function Render() {
		const [chosen, setChosen] = useState("workspace");
		return (
			<RowGroup>
				{sources.map(({ id, title, description }) => (
					<Cell
						key={id}
						render={
							// oxlint-disable-next-line jsx_a11y/control-has-associated-label -- The row renders as this button, and Cell fills it with the title.
							<button type="button" aria-pressed={chosen === id} onClick={() => setChosen(id)} />
						}
						title={title}
						description={description}
						trailing={chosen === id ? <Icon name="tick" /> : undefined}
					/>
				))}
			</RowGroup>
		);
	},
});

const machines = [
	{ name: "macbook", online: true, repos: 6, here: true },
	{ name: "closet-pc", online: true, repos: 3 },
	{ name: "old-mac", online: false, repos: 2, seen: "2d ago" },
];

/** Machines with their light and what they hold; an offline one says when it was last seen. */
export const Machines = meta.story({
	render: () => (
		<RowGroup>
			{machines.map(({ name, online, repos, seen, here }) => (
				<Cell
					key={name}
					leading={<Icon name="laptop" size={28} />}
					title={name}
					online={online}
					badge={here === true ? <Badge variant="lightGray">This one</Badge> : undefined}
					meta={
						<>
							<span>{repos} repos</span>
							{seen !== undefined && (
								<span style={{ display: "flex", alignItems: "center", gap: 4 }}>
									<Icon name="clock" />
									{seen}
								</span>
							)}
						</>
					}
				/>
			))}
		</RowGroup>
	),
});
