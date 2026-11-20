import { stories } from "../stories/stories.ts";
import { MetaCount, MetaStatus, type MetaStatusVariant } from "./MetaCount.tsx";

const core = (node: string) =>
	`https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=${node}`;

const meta = stories({
	title: "Content & status/MetaStatus",
	id: "components-metastatus",
	component: MetaStatus,
	parameters: { design: { type: "figma", url: core("2572-3595") } },
	args: { variant: "passed", children: "CI" },
});

/** A pull request's checks, passed: the glyph and the colour say the state, the label the thing. */
export const Default = meta.story({ args: { variant: "passed", children: "CI" } });

const variants: Array<MetaStatusVariant> = ["working", "passed", "failed", "warning", "neutral"];

/** Each state on the same label: running, passed, failed, waiting on you, and no result. */
export const Variants = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 12 }}>
			{variants.map((variant) => (
				<MetaStatus key={variant} variant={variant}>
					CI
				</MetaStatus>
			))}
		</div>
	),
});

/** Other things that run, named by what they are: a deploy, a build, a sync. */
export const Labels = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 12 }}>
			<MetaStatus variant="passed">Deploy</MetaStatus>
			<MetaStatus variant="working">Build</MetaStatus>
			<MetaStatus variant="warning">Sync</MetaStatus>
		</div>
	),
});

/** The glyph alone, where what it is about is plain, named for a screen reader by `aria-label`. */
export const WithoutLabel = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 12 }}>
			<MetaStatus variant="working" aria-label="Checks running" />
			<MetaStatus variant="passed" aria-label="Checks passed" />
			<MetaStatus variant="failed" aria-label="Checks failed" />
			<MetaStatus variant="warning" aria-label="Checks need you" />
			<MetaStatus variant="neutral" aria-label="Checks pending" />
		</div>
	),
});

/** Checks with how far they got, as a pull request's tab shows them: 2 of 14 done, or failed. */
export const WithProgress = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 12 }}>
			<MetaStatus variant="working">CI 2/14</MetaStatus>
			<MetaStatus variant="failed">CI 2/14</MetaStatus>
			<MetaStatus variant="passed">CI</MetaStatus>
		</div>
	),
});

/** In a branch row's meta, first after conflicts and before the counts, as `BranchItem` reads it. */
export const InRowMeta = meta.story({
	render: () => (
		<div style={{ display: "flex", alignItems: "center" }}>
			<MetaStatus variant="working">CI</MetaStatus>
			<MetaCount type="behind">3</MetaCount>
			<MetaCount type="unpushed">4</MetaCount>
		</div>
	),
});
