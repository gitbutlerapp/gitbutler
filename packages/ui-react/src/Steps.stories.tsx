import { stories } from "../docs/stories.ts";
import { CodeBlock } from "./CodeBlock.tsx";
import { Step, Steps } from "./Steps.tsx";

const figma = (nodeId: string) => ({
	design: {
		type: "figma",
		url: `https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=${nodeId}`,
	},
});

const meta = stories({
	title: "Content & status/Steps",
	id: "components-steps",
	component: Steps,
	parameters: figma("4686-216442"),
	decorators: [
		(Story) => (
			<div style={{ maxWidth: 386 }}>
				<Story />
			</div>
		),
	],
});

const copyToClipboard = (value: string) => void navigator.clipboard.writeText(value);

export const Default = meta.story({
	args: {
		children: (
			<>
				<Step
					title="Sign in to Vercel on a connected computer"
					description="Each task runs in its own sandbox in your Vercel account."
				/>
				<Step title="Start a task from a worktree there" />
			</>
		),
	},
});

/** What each step needs, under its title: here a command to run. */
export const Instructions = meta.story({
	parameters: figma("4701-2914"),
	render: () => (
		<Steps>
			<Step title="Sign in to Vercel on a connected computer">
				<CodeBlock prompt onCopy={copyToClipboard}>
					vercel login
				</CodeBlock>
			</Step>
			<Step title="Start a task from a worktree there">
				<CodeBlock prompt onCopy={copyToClipboard}>
					{'but mesh run "add tests" --on vercel'}
				</CodeBlock>
			</Step>
		</Steps>
	),
});

/** A step that passed, the one the user is on, and one still to come. */
export const Progress = meta.story({
	parameters: figma("4701-2963"),
	render: () => (
		<Steps>
			<Step
				title="Install the CLI"
				description="One command on the computer you are connecting."
				variant="safe"
				done
			/>
			<Step
				title="Approve this computer"
				description="Open the link the CLI printed and approve the code it shows."
				variant="fillGray"
			/>
			<Step title="Add a repository" description="Run any but command in a repository there." />
		</Steps>
	),
});
