import preview from "#storybook/preview";
import { DiffFile } from "./DiffFile.tsx";
import { DiffFileHeader } from "./DiffFileHeader.tsx";
import { DiffFileList } from "./DiffFileList.tsx";

const meta = preview.meta({
	title: "Content & status/DiffFileList",
	id: "components-difffilelist",
	component: DiffFileList,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=2425-6005",
		},
	},
});

const files = [
	{ path: "src/components/Button.svelte", added: 4, removed: 3 },
	{ path: "src/lib/router.ts", added: 12, removed: 0 },
	{ path: "src/routes/+layout.svelte", added: 1, removed: 8 },
];

/**
 * Cards spaced the way every app spaces its diffs. The folded file shows a
 * card that is only its header.
 */
export const Default = meta.story({
	render: () => (
		<DiffFileList style={{ width: 546 }}>
			{files.map((file, index) => (
				<DiffFile
					key={file.path}
					collapsed={index === 1}
					header={
						<DiffFileHeader
							path={file.path}
							added={file.added}
							removed={file.removed}
							collapsed={index === 1}
							onCollapsedChange={() => {}}
						/>
					}
				>
					<pre
						className="text-12"
						style={{ margin: 0, padding: "4px 12px", fontFamily: "var(--font-family-mono)" }}
					>
						{"const router = useRouter()\nconst theme = useTheme()"}
					</pre>
				</DiffFile>
			))}
		</DiffFileList>
	),
});
