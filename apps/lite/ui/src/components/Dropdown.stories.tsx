import preview from "#storybook/preview";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { Dropdown, PopupItem, PopupSearch, PopupSection } from "./Popup.tsx";

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

export const Playground = meta.story({
	args: {
		"aria-label": "Dropdown playground",
		style: { width: 240 },
		trigger: (
			<button type="button" className={getButtonClassName({})}>
				Open dropdown
			</button>
		),
		children: (
			<PopupSection>
				<PopupItem icon="branch">Switch branch</PopupItem>
				<PopupItem icon="commit">Amend commit</PopupItem>
				<PopupItem icon="bin">Discard changes</PopupItem>
			</PopupSection>
		),
	},
});

/** The target selector: a filter over a short list, anchored under the control that opened it. */
export const WithSearch = meta.story({
	args: {
		"aria-label": "Select target",
		style: { width: 256 },
		trigger: (
			<button type="button" className={getButtonClassName({})}>
				Select target
			</button>
		),
		children: (
			<>
				<PopupSearch placeholder="Search targets..." aria-label="Search targets" />
				<PopupSection>
					<PopupItem icon="branch" trailing="bullseye">
						rocketFlasher
					</PopupItem>
					<PopupItem icon="branch">Fliege-mono</PopupItem>
					<PopupItem icon="branch">brutalism</PopupItem>
				</PopupSection>
			</>
		),
	},
});

/** A panel rather than a list — what the notifications bell opens. */
export const Panel = meta.story({
	args: {
		"aria-label": "Notifications",
		style: { width: 380 },
		trigger: (
			<button type="button" className={getButtonClassName({ iconOnly: true })}>
				<Icon name="bell" />
			</button>
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
