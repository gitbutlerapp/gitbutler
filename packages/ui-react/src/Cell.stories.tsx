import { stories } from "../docs/stories.ts";
import { useState } from "react";
import { Badge } from "./Badge.tsx";
import { Cell } from "./Cell.tsx";
import { RowGroup } from "./RowGroup.tsx";
import { Switch } from "./Switch.tsx";

const figma = (nodeId: string) => ({
	design: {
		type: "figma",
		url: `https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=${nodeId}`,
	},
});

const meta = stories({
	title: "Content & status/Cell",
	id: "components-cell",
	component: Cell,
	parameters: figma("4537-7201"),
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
		title: "Whole workspace",
		icon: "commit",
		description: "10 branches in 6 stacks",
		badge: <Badge variant="lightGray">12</Badge>,
	},
});

/** A row that opens something: the whole row is the link, with the chevron at the end. */
export const OpensSomething = meta.story({
	render: () => (
		<RowGroup>
			<Cell
				// oxlint-disable-next-line jsx_a11y/anchor-has-content, jsx_a11y/control-has-associated-label -- The row renders as this link, and Cell fills it with the title.
				render={<a href="#repositories" />}
				icon="repo"
				title="Repositories"
				badge={<Badge variant="lightGray">12</Badge>}
				description="The ones your machines have checked out."
				trailing="chevron"
			/>
		</RowGroup>
	),
});

const BlockedSetting = () => {
	const [on, setOn] = useState(false);
	return (
		<Cell
			// oxlint-disable-next-line jsx_a11y/label-has-associated-control -- The row renders as this label, and Cell fills it with the title.
			render={<label htmlFor="open-prs" />}
			title="Open pull requests"
			description="Connect GitHub first, so agents can open them."
			disabled
			trailing={<Switch id="open-prs" size="large" checked={on} disabled onCheckedChange={setOn} />}
		/>
	);
};

/** A setting that can't be turned on yet: the row fades on its grey ground, and says why. */
export const Disabled = meta.story({
	render: () => (
		<RowGroup>
			<BlockedSetting />
		</RowGroup>
	),
});
