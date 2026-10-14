import preview from "#storybook/preview";
import { useState } from "react";
import { DiffFile } from "./DiffFile.tsx";
import { DiffFileHeader, type DiffFileReviewState } from "./DiffFileHeader.tsx";
import { DiffFileList } from "./DiffFileList.tsx";
import { ScrollArea } from "./ScrollArea.tsx";

type Line = { kind: " " | "+" | "-"; text: string };

const sampleLines: Array<Line> = [
	{ kind: " ", text: "const router = useRouter()" },
	{ kind: "+", text: "const [isOpen, setIsOpen] = useState(false)" },
	{ kind: "+", text: "const ref = useRef<HTMLDivElement>(null)" },
	{ kind: " ", text: "const theme = useTheme()" },
	{ kind: "-", text: "let data = fetchData(endpoint, opts)" },
	{ kind: "-", text: "return <Wrapper className={styles.root}>" },
	{ kind: " ", text: "export default function Handler() {" },
	{ kind: "+", text: '  if (loading) return <Spinner size="sm" />' },
	{ kind: "-", text: "  if (loading) return null" },
	{ kind: " ", text: "}" },
];

const lineBackground = { " ": undefined, "+": "var(--bg-safe)", "-": "var(--bg-danger)" };

/**
 * Stands in for the host's diff renderer (Pierre in Lite and but.dev), which
 * the library doesn't depend on.
 */
const SampleDiff = ({ lines = sampleLines }: { lines?: Array<Line> }) => (
	<pre
		className="text-12"
		style={{ margin: 0, paddingBlock: 4, fontFamily: "var(--font-family-mono)" }}
	>
		{lines.map((line, index) => (
			<div
				// oxlint-disable-next-line react/no-array-index-key -- A fixed sample; lines repeat.
				key={index}
				style={{ paddingInline: 12, lineHeight: "20px", background: lineBackground[line.kind] }}
			>
				{line.kind} {line.text}
			</div>
		))}
	</pre>
);

const meta = preview.meta({
	title: "Content & status/DiffFile",
	id: "components-difffile",
	component: DiffFile,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=2415-6323",
		},
	},
	args: {
		collapsed: false,
		header: <DiffFileHeader path="src/components/Button.svelte" added={4} removed={3} />,
	},
	decorators: [
		(Story) => (
			<div style={{ width: 546 }}>
				<Story />
			</div>
		),
	],
});

export const Default = meta.story({
	args: {
		children: <SampleDiff />,
	},
});

/** Folding keeps the header and drops the diff; the header's divider goes with it. */
export const Folded = meta.story({
	render: function Render() {
		const [collapsed, setCollapsed] = useState(true);

		return (
			<DiffFile
				collapsed={collapsed}
				header={
					<DiffFileHeader
						path="src/components/Button.svelte"
						added={4}
						removed={3}
						collapsed={collapsed}
						onCollapsedChange={setCollapsed}
					/>
				}
			>
				<SampleDiff />
			</DiffFile>
		);
	},
});

const files = [
	"src/components/Button.svelte",
	"src/lib/router.ts",
	"src/routes/+layout.svelte",
	"src/lib/stores/theme.ts",
];

/**
 * A branch's diff, one card per file in a scroller. Scroll it: each header
 * holds the top while its file passes; as the file leaves, its card shrinks
 * around the header until the next file takes the top.
 */
export const Files = meta.story({
	render: function Render() {
		const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(() => new Set());
		const [reviewed, setReviewed] = useState<ReadonlySet<string>>(
			() => new Set(["src/lib/router.ts"]),
		);
		const toggle = (set: ReadonlySet<string>, path: string, on: boolean): ReadonlySet<string> => {
			const next = new Set(set);
			if (on) next.add(path);
			else next.delete(path);
			return next;
		};

		return (
			<ScrollArea style={{ height: 420 }}>
				<DiffFileList>
					{files.map((path) => {
						const isCollapsed = collapsed.has(path);
						const reviewState: DiffFileReviewState = reviewed.has(path) ? "reviewed" : "unreviewed";

						return (
							<DiffFile
								key={path}
								collapsed={isCollapsed}
								header={
									<DiffFileHeader
										path={path}
										added={4}
										removed={3}
										collapsed={isCollapsed}
										onCollapsedChange={(on) => setCollapsed((set) => toggle(set, path, on))}
										reviewState={reviewState}
										onReviewedChange={(on) => setReviewed((set) => toggle(set, path, on))}
										onMenu={() => {}}
									/>
								}
							>
								<SampleDiff lines={[...sampleLines, ...sampleLines]} />
							</DiffFile>
						);
					})}
				</DiffFileList>
			</ScrollArea>
		);
	},
});
