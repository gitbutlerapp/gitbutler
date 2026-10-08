import preview from "#storybook/preview";
import { Button } from "./Button.tsx";
import { Icon } from "./Icon.tsx";
import { Dropdown, PopupItem, PopupSection } from "./Popup.tsx";

const meta = preview.meta({
	component: Dropdown,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=1819-3456",
		},
	},
	args: {
		side: "bottom",
		align: "start",
		sideOffset: 4,
	},
	argTypes: {
		side: { control: "inline-radio", options: ["top", "bottom", "left", "right"] },
		align: { control: "inline-radio", options: ["start", "center", "end"] },
	},
	decorators: [
		(Story) => (
			<div style={{ display: "flex", justifyContent: "center", padding: "120px 48px" }}>
				<Story />
			</div>
		),
	],
});

/**
 * A filter anchored under its control: each row shows or hides one kind, ticked while shown. Not a
 * menu of actions — that is a `Menu`.
 */
export const Default = meta.story({
	args: {
		"aria-label": "Show",
		trigger: <Button>Filter</Button>,
		children: (
			<PopupSection>
				<PopupItem aria-pressed trailing="tick">
					Local branches
				</PopupItem>
				<PopupItem aria-pressed trailing="tick">
					Remote branches
				</PopupItem>
				<PopupItem aria-pressed={false}>Pull requests</PopupItem>
			</PopupSection>
		),
	},
});

/** A panel rather than rows: `children` can be anything, and the container does not change for it. */
export const Panel = meta.story({
	args: {
		"aria-label": "Notifications",
		style: { width: 380 },
		trigger: (
			<Button iconOnly>
				<Icon name="bell" />
			</Button>
		),
		children: (
			<div style={{ display: "flex", flexDirection: "column", gap: 8, padding: 12 }}>
				<strong className="text-12 text-semibold">Notifications</strong>
				<span className="text-13">
					A dropdown carries anchored panels as readily as it carries rows — the container does not
					care what fills it.
				</span>
			</div>
		),
	},
});
