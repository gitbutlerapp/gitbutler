import preview from "#storybook/preview";
import { ProgramIcon } from "./ProgramIcon.tsx";

// Read off the directory rather than exported from `programIcons.ts`, which keeps the
// module's only export the lookup the app uses.
const programs = Object.keys(import.meta.glob("./program-icons/*.png"))
	.map((path) => path.replace(/^.*\/(.+)@2x\.png$/, "$1"))
	.sort((a, b) => a.localeCompare(b));

const meta = preview.type<{ args: { size: number } }>().meta({
	argTypes: {
		size: {
			control: { type: "range", min: 8, max: 64, step: 2 },
		},
	},
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=2044-2861",
		},
	},
});

/** Every mark the library has drawn, at the size a select's slot renders it. */
export const AllPrograms = meta.story({
	args: { size: 14 },
	render: (args) => (
		<div style={{ display: "flex", flexWrap: "wrap", gap: 16, padding: 24 }}>
			{programs.map((program) => (
				<div
					key={program}
					className="text-11"
					style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 6 }}
				>
					<ProgramIcon program={program} size={args.size} />
					<span>{program}</span>
				</div>
			))}
		</div>
	),
});
