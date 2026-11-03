import { stories } from "../docs/stories.ts";
import { Code } from "./Code.tsx";
import { EmptyState } from "./EmptyState.tsx";

const figma = (nodeId: string) => ({
	design: {
		type: "figma",
		url: `https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=${nodeId}`,
	},
});

const meta = stories({
	title: "Content & status/Code",
	id: "components-code",
	component: Code,
	parameters: figma("4633-925"),
});

export const Default = meta.story({
	args: {
		children: "but mesh service restart",
	},
});

/** A command in a sentence, where it reads as part of the text. */
export const InText = meta.story({
	parameters: figma("4633-1011"),
	render: () => (
		<p className="text-13" style={{ color: "var(--text-2)" }}>
			Run <Code>but config forge auth</Code> there to connect GitHub.
		</p>
	),
});

/** An empty state that says how to get out of it. */
export const InEmptyState = meta.story({
	render: () => (
		<EmptyState
			illustration="cactus"
			title="scott--mac-studio is offline"
			description={
				<>
					Last seen 1d ago. Wake it up or run <Code>but mesh service restart</Code> there; then its
					uncommitted changes can be loaded here.
				</>
			}
		/>
	),
});
