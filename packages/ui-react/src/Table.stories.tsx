import { stories } from "../docs/stories.ts";
import { Button } from "./Button.tsx";
import { Code } from "./Code.tsx";
import { Icon } from "./Icon.tsx";
import { StatusLed } from "./StatusLed.tsx";
import { Switch } from "./Switch.tsx";
import { Table, TableCell, TableRow } from "./Table.tsx";
import { TextLink } from "./TextLink.tsx";

const figma = (nodeId: string) => ({
	design: {
		type: "figma",
		url: `https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=${nodeId}`,
	},
});

const meta = stories({
	title: "Content & status/Table",
	id: "components-table",
	component: Table,
	parameters: figma("4799-1082"),
	decorators: [
		(Story) => (
			<div style={{ maxWidth: 720 }}>
				<Story />
			</div>
		),
	],
});

export const Default = meta.story({
	args: {
		header: (
			<TableRow>
				<TableCell header>File</TableCell>
				<TableCell header>Change</TableCell>
				<TableCell header>Status</TableCell>
			</TableRow>
		),
		children: (
			<>
				<TableRow>
					<TableCell>Markdown.tsx</TableCell>
					<TableCell>Description editor</TableCell>
					<TableCell>Done</TableCell>
				</TableRow>
				<TableRow>
					<TableCell>Field.tsx</TableCell>
					<TableCell>Empty prompt</TableCell>
					<TableCell>In review</TableCell>
				</TableRow>
			</>
		),
	},
});

/** An index of pages, each a link with when to read it. */
export const DocsIndex = meta.story({
	parameters: figma("4801-1625"),
	render: () => (
		<Table
			header={
				<TableRow>
					<TableCell header>Page</TableCell>
					<TableCell header>Read it when</TableCell>
				</TableRow>
			}
		>
			{[
				["Emphasis", "Choosing which control stands out, and in what color"],
				["States", "Anything clickable: its hover and focus"],
				["Radius", "A corner, especially one inside another"],
				["Motion", "A transition or an animation"],
			].map(([page, when]) => (
				<TableRow key={page}>
					<TableCell>
						<TextLink href="#">{page}</TextLink>
					</TableCell>
					<TableCell>{when}</TableCell>
				</TableRow>
			))}
		</Table>
	),
});

/** Keys with a name to explain, the key as typed, dates, a switch, and a menu to end the row. */
export const ApiKeys = meta.story({
	parameters: figma("4801-1917"),
	render: () => (
		<Table
			header={
				<TableRow>
					<TableCell header>Name</TableCell>
					<TableCell header>Key</TableCell>
					<TableCell header>Created</TableCell>
					<TableCell header>Expires</TableCell>
					<TableCell header>Enabled</TableCell>
					<TableCell header align="end" aria-label="Actions" />
				</TableRow>
			}
		>
			{[
				{ name: "room-tone", key: "••••••d6d6", created: "Nov 3", expires: "Never", on: true },
				{
					name: "release-bot",
					key: "••••••41aa",
					created: "Oct 18",
					expires: "Jan 18, 2027",
					on: false,
				},
			].map((key) => (
				<TableRow key={key.name}>
					<TableCell>
						{key.name} <Icon name="info" />
					</TableCell>
					<TableCell>
						<Code>{key.key}</Code>
					</TableCell>
					<TableCell>{key.created}</TableCell>
					<TableCell>{key.expires}</TableCell>
					<TableCell>
						<Switch defaultChecked={key.on} aria-label={`Enable ${key.name}`} />
					</TableCell>
					<TableCell align="end">
						<Button variant="ghost" size="small" iconOnly aria-label={`More for ${key.name}`}>
							<Icon name="kebab" />
						</Button>
					</TableCell>
				</TableRow>
			))}
		</Table>
	),
});

/** Machines with their light, a count lined up on its last digit, and when each was last seen. */
export const Machines = meta.story({
	parameters: figma("4801-2237"),
	render: () => (
		<Table
			header={
				<TableRow>
					<TableCell header>Machine</TableCell>
					<TableCell header align="end">
						Branches
					</TableCell>
					<TableCell header>Last seen</TableCell>
					<TableCell header align="end" aria-label="Actions" />
				</TableRow>
			}
		>
			{[
				{ name: "macbook-pro", online: true, branches: 4, seen: "Now" },
				{ name: "mac-studio", online: true, branches: 12, seen: "Now" },
				{ name: "build-runner-2", online: false, branches: 0, seen: "3 days ago" },
			].map((machine) => (
				<TableRow key={machine.name}>
					<TableCell>
						<StatusLed online={machine.online} /> {machine.name}
					</TableCell>
					<TableCell align="end">{machine.branches}</TableCell>
					<TableCell>{machine.seen}</TableCell>
					<TableCell align="end">
						<Button variant="ghost" size="small" iconOnly aria-label={`More for ${machine.name}`}>
							<Icon name="kebab" />
						</Button>
					</TableCell>
				</TableRow>
			))}
		</Table>
	),
});
