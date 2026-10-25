import preview from "#storybook/preview";
import { MetaCount } from "./MetaCount.tsx";
import { MetaPart, MetaWord, type MetaPartTone } from "./MetaPart.tsx";
import { ViewHeader } from "./ViewHeader.tsx";

const tones: Array<MetaPartTone> = ["neutral", "green", "amber", "blue", "red"];

const meta = preview.meta({
	title: "Preview/MetaPart",
	id: "components-metapart",
	component: MetaPart,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=3006-44182",
		},
	},
	argTypes: {
		tone: { control: "select", options: tones },
	},
	args: { icon: "pr", tone: "neutral", children: "2 PRs" },
});

/** One part: its glyph, then its words, in the tone of whose move it is. */
export const Default = meta.story({ args: { icon: "pr", children: "2 PRs" } });

/** Each tone, on a part it suits. */
export const Tones = meta.story({
	render: () => (
		<div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
			<MetaPart icon="tag">CLI 0.2.1662</MetaPart>
			<MetaPart icon="tick" tone="green">
				CLI 0.2.1662
			</MetaPart>
			<MetaPart icon="arrow-up" tone="amber">
				CLI 0.2.1521 outdated
			</MetaPart>
			<MetaPart icon="arrow-up" tone="blue">
				4 unpushed
			</MetaPart>
			<MetaPart icon="pr" tone="red">
				2 PRs · 1 failing
			</MetaPart>
		</div>
	),
});

/** With `href` it opens what it describes, and only then are its words underlined. */
export const Link = meta.story({
	args: { icon: "pr", tone: "red", href: "#", children: "2 PRs · 1 failing" },
});

const RepoHeader = () => (
	<ViewHeader
		title="but-dev"
		meta={
			<>
				<MetaCount type="agent-working">
					3<MetaWord> agents working</MetaWord>
				</MetaCount>
				<MetaPart icon="pr" tone="red" href="#">
					2<MetaWord> PRs</MetaWord> · 1<MetaWord> failing</MetaWord>
				</MetaPart>
				<MetaCount type="branch">review-cards 5m</MetaCount>
			</>
		}
	/>
);

/**
 * A repo's header in but.dev, MetaParts beside MetaCounts in a ViewHeader's meta line. The pane is
 * a size container 820px wide, so the words show.
 */
export const InAHeader = meta.story({
	render: () => (
		<div style={{ width: 820, containerType: "inline-size", backgroundColor: "var(--bg-1)" }}>
			<RepoHeader />
		</div>
	),
});

/** The same header in a pane under 640px wide: each MetaWord drops, the glyphs and numbers stay. */
export const NarrowPane = meta.story({
	render: () => (
		<div style={{ width: 420, containerType: "inline-size", backgroundColor: "var(--bg-1)" }}>
			<RepoHeader />
		</div>
	),
});
