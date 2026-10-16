import preview from "#storybook/preview";
import { Code } from "./Code.tsx";
import { EmptyState } from "./EmptyState.tsx";

const meta = preview.meta({
	title: "Content & status/Code",
	id: "components-code",
	component: Code,
});

export const Default = meta.story({
	args: {
		children: "but mesh service restart",
	},
});

/** A command in a sentence, where it reads as part of the text. */
export const InText = meta.story({
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
