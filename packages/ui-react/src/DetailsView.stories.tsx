import preview from "#storybook/preview";
import { Toggle, ToggleGroup } from "@base-ui/react";
import type { FC, ReactNode } from "react";
import { Breadcrumbs, type Crumb } from "./Breadcrumbs.tsx";
import { Button } from "./Button.tsx";
import { DiffFile } from "./DiffFile.tsx";
import { DiffFileHeader } from "./DiffFileHeader.tsx";
import { DiffFileList } from "./DiffFileList.tsx";
import { FileList, FileListItem } from "./FileList.tsx";
import { Icon } from "./Icon.tsx";
import { LocationBar, LocationBarDivider } from "./LocationBar.tsx";
import { MetaCount } from "./MetaCount.tsx";
import { ScrollArea } from "./ScrollArea.tsx";
import { SwitchButton } from "./SwitchButton.tsx";
import { Tab, Tabs } from "./Tabs.tsx";
import { ToggleGroupStyles, ToggleStyles } from "./ToggleGroup.tsx";
import { ViewHeader, ViewHeaderButton, ViewHeaderDivider } from "./ViewHeader.tsx";

const meta = preview.meta({
	title: "Preview/Details view",
	id: "preview-details-view",
	parameters: {
		layout: "fullscreen",
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=3249-51240",
		},
	},
});

type Line = { kind: " " | "+" | "-"; text: string };

const lines: Array<Line> = [
	{ kind: " ", text: "@Expose({ name: 'pushRemoteUrl' })" },
	{ kind: "+", text: "style:left={inputBoundingRect?.left ?" },
	{ kind: "+", text: "style:max-height={maxHeightState && `${maxHeightState}`}" },
	{ kind: "-", text: "@Transform(({ value }: TransformFnParams) => (value ?" },
	{ kind: " ", text: "value={options.find((item) => item.value ===" },
	{ kind: " ", text: "disabled={disabled || loading}" },
	{ kind: "-", text: '<div class="option" tabindex="-1" role="none">' },
	{ kind: "-", text: "  {@render itemSnippet({ item, highlighted: idx === hi" },
	{ kind: "-", text: "</div>" },
	{ kind: " ", text: "const router = useRouter()" },
	{ kind: "+", text: "const [isOpen, setIsOpen] = useState(false)" },
	{ kind: "+", text: "const ref = useRef<HTMLDivElement>(null)" },
	{ kind: " ", text: "const theme = useTheme()" },
	{ kind: "-", text: "let data = fetchData(endpoint, opts)" },
	{ kind: " ", text: "export default function Handler() {" },
	{ kind: "+", text: '  if (loading) return <Spinner size="sm" />' },
	{ kind: "-", text: "  if (loading) return null" },
	{ kind: " ", text: "}" },
];

const lineBackground = { " ": undefined, "+": "var(--bg-safe)", "-": "var(--bg-danger)" };

/** Stands in for the host's diff renderer (Pierre in Lite and but.dev). */
const SampleDiff: FC = () => (
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

const Live: FC = () => (
	<>
		<span style={{ display: "inline-flex", alignItems: "center", columnGap: 6 }}>
			<span
				aria-hidden
				style={{ width: 6, height: 6, borderRadius: "50%", backgroundColor: "var(--text-safe)" }}
			/>
			live
		</span>
		<Button variant="ghost" iconOnly aria-label="Refresh">
			<Icon name="refresh" />
		</Button>
	</>
);

const kebab = (label: string) => (
	<Button variant="ghost" iconOnly aria-label={label}>
		<Icon name="kebab" />
	</Button>
);

const SendAndMenu: FC = () => (
	<>
		<ViewHeaderButton>
			Send to
			<Icon name="arrow-corner-up-right" />
		</ViewHeaderButton>
		<ViewHeaderButton variant="ghost" iconOnly aria-label="More">
			<Icon name="kebab" />
		</ViewHeaderButton>
	</>
);

/** The pane under the location bar and the header: the changed files, and their diff. */
const Changes: FC = () => (
	<div style={{ display: "flex", flex: 1, minHeight: 0 }}>
		<div
			style={{
				display: "flex",
				flexDirection: "column",
				flexShrink: 0,
				width: 318,
				borderInlineEnd: "1px solid var(--border-section)",
			}}
		>
			<FileList
				title="Changes"
				count={10}
				added={34}
				removed={28}
				onOpenFilter={() => {}}
				actions={kebab("Changes menu")}
				focused
			>
				<FileListItem name="useAuthHook.ts" directory="src/hooks" status="Addition" />
				<FileListItem
					selected
					name="Button.tsx"
					directory="src/components"
					status="Modification"
					actions={kebab("File menu")}
				/>
				<FileListItem name="apiService.ts" directory="src/services" status="Modification" />
				<FileListItem name="styles.css" directory="src/styles" status="Addition" />
				<FileListItem name="theme.css" directory="src/styles" status="Modification" />
				<FileListItem name="index.ts" directory="src" status="Deletion" />
			</FileList>
		</div>

		<div style={{ display: "flex", flex: 1, flexDirection: "column", minWidth: 0 }}>
			{/* The diff's bar: 12px in from the pane's top and sides, as the file list's header. */}
			<div
				style={{
					display: "flex",
					columnGap: "var(--gap-buttons-compact)",
					alignItems: "center",
					padding: "var(--padding-panel) var(--padding-panel) 0",
				}}
			>
				<Button variant="ghost" iconOnly aria-label="Hide the file list">
					<Icon name="files-sidebar" />
				</Button>
				<span style={{ flex: 1 }} />
				<Button>Mark all viewed</Button>
				<ToggleGroup render={<ToggleGroupStyles />} defaultValue={["contain"]} aria-label="Lines">
					<Toggle render={<ToggleStyles iconOnly />} value="wrap" aria-label="Wrap lines">
						<Icon name="text-wrap" />
					</Toggle>
					<Toggle render={<ToggleStyles iconOnly />} value="contain" aria-label="Scroll lines">
						<Icon name="text-contain" />
					</Toggle>
				</ToggleGroup>
				<ToggleGroup render={<ToggleGroupStyles />} defaultValue={["unified"]} aria-label="Layout">
					<Toggle render={<ToggleStyles />} value="split">
						Split
					</Toggle>
					<Toggle render={<ToggleStyles />} value="unified">
						Unified
					</Toggle>
				</ToggleGroup>
			</div>
			<ScrollArea style={{ flex: 1 }}>
				<DiffFileList>
					{["src/components/Button.svelte", "src/components/Tooltip.svelte"].map((path) => (
						<DiffFile
							key={path}
							header={
								<DiffFileHeader
									path={path}
									added={34}
									removed={28}
									onCollapsedChange={() => {}}
									reviewState="unreviewed"
									onReviewedChange={() => {}}
									onMenu={() => {}}
								/>
							}
						>
							<SampleDiff />
						</DiffFile>
					))}
				</DiffFileList>
			</ScrollArea>
		</div>
	</div>
);

const Frame: FC<{ crumbs: Array<Crumb>; state: ReactNode; header: ReactNode }> = ({
	crumbs,
	state,
	header,
}) => (
	<div
		style={{
			display: "flex",
			flexDirection: "column",
			width: 846,
			height: 760,
			overflow: "hidden",
			backgroundColor: "var(--bg-1)",
		}}
	>
		<LocationBar
			leading={
				<>
					<Button variant="ghost" iconOnly aria-label="Toggle sidebar">
						<Icon name="sidebar" />
					</Button>
					<LocationBarDivider />
					<Button variant="ghost" iconOnly aria-label="Back">
						<Icon name="arrow-left" />
					</Button>
				</>
			}
			state={state}
		>
			<Breadcrumbs items={crumbs} />
		</LocationBar>
		{header}
		<Changes />
	</div>
);

/**
 * A branch, the view with the most in it: its tabs and the commit filter make the header's
 * toolbar, and its buttons stand beside the title. Under it, the file list and the diff, each
 * 12px in from the pane.
 */
export const Default = meta.story({
	render: () => (
		<Frame
			crumbs={[{ label: "mesh", href: "#mesh" }]}
			state={<Live />}
			header={
				<ViewHeader
					icon="branch"
					title="ios-push-handoff"
					meta={
						<>
							<MetaCount type="commits">3 commits ahead</MetaCount>
							<MetaCount type="agent-working">Agent working 12m</MetaCount>
						</>
					}
					toolbar={
						<>
							<Tabs defaultValue="diff" aria-label="Branch tab">
								<Tab value="diff" icon={<Icon name="diff" />}>
									Diff
								</Tab>
								<Tab value="pr" icon={<Icon name="pr" />}>
									Pull Request
								</Tab>
							</Tabs>
							<ViewHeaderDivider />
							<Button>
								<Icon name="commit" />
								All 4 commits
							</Button>
						</>
					}
					actions={
						<>
							<SwitchButton label="Auto-merge" variant="outline" />
							<SendAndMenu />
						</>
					}
				/>
			}
		/>
	),
});

/** A worktree has no views to switch, so no toolbar: its buttons stand beside the title block. */
export const Worktree = meta.story({
	render: () => (
		<Frame
			crumbs={[
				{ label: "mesh", href: "#mesh" },
				{ label: "pavel--macbook-pro-2", href: "#machine" },
				{ label: "but-dev", href: "#repo" },
			]}
			state={
				<>
					<span className="text-13" style={{ color: "var(--text-2)" }}>
						Also in 2 places · <span style={{ color: "var(--text-warn)" }}>1 differs</span>
					</span>
					<Live />
				</>
			}
			header={
				<ViewHeader
					icon="folder"
					title="Main worktree"
					meta={
						<>
							<MetaCount type="behind">3 upstream</MetaCount>
							<MetaCount type="uncommitted">3 files</MetaCount>
							<MetaCount type="agent-working">3 agents working</MetaCount>
							<MetaCount type="age">3d</MetaCount>
						</>
					}
					actions={<SendAndMenu />}
				/>
			}
		/>
	),
});
