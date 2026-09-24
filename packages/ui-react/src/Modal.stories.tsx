import preview from "#storybook/preview";
import { Button } from "./Button.tsx";
import { Modal, PopupItem, PopupSearch, PopupSection } from "./Popup.tsx";

const meta = preview.meta({
	component: Modal,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=1819-3456",
		},
	},
	args: {
		size: "small",
		align: "center",
		alert: false,
		trigger: <Button>Open modal</Button>,
	},
	argTypes: {
		size: { control: "inline-radio", options: ["small", "medium", "large"] },
		align: { control: "inline-radio", options: ["center", "top"] },
		alert: { control: "boolean" },
	},
	decorators: [
		(Story) => (
			<div style={{ display: "flex", justifyContent: "center", padding: "48px" }}>
				<Story />
			</div>
		),
	],
});

export const Playground = meta.story({
	args: {
		"aria-label": "Modal playground",
		children: (
			<div style={{ display: "flex", flexDirection: "column", gap: 12, padding: 16 }}>
				<strong className="text-15 text-semibold">Git credentials required</strong>
				<span className="text-13">
					An alert modal takes the `alertdialog` role and refuses Escape and backdrop clicks — the
					question has to be answered rather than dismissed.
				</span>
				<div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
					<Button variant="ghost">Cancel</Button>
					<Button variant="pop">Continue</Button>
				</div>
			</div>
		),
	},
});

/** A picker: top-aligned so its list grows downward, and filled with the popup's own parts. */
export const Picker = meta.story({
	args: {
		size: "small",
		align: "top",
		"aria-label": "Select project",
		trigger: <Button>Select project</Button>,
		children: (
			<>
				<PopupSearch placeholder="Search projects..." aria-label="Search projects" />
				<PopupSection label="Recent projects">
					<PopupItem icon="folder-tree" trailing="tick">
						rocketFlasher
					</PopupItem>
					<PopupItem icon="lock">Fliege-mono</PopupItem>
					<PopupItem icon="folder-tree">brutalism</PopupItem>
				</PopupSection>
				<PopupSection>
					<PopupItem trailing="plus">Add local repository</PopupItem>
					<PopupItem trailing="copy">Clone repository</PopupItem>
				</PopupSection>
			</>
		),
	},
});

/** The settings-sized pane: the modal supplies the chrome, the caller lays out everything inside. */
export const Large = meta.story({
	args: {
		size: "large",
		"aria-label": "Resolve conflicts",
		trigger: <Button>Resolve conflicts</Button>,
		children: (
			<div style={{ height: 400, padding: 16 }}>
				<span className="text-13">
					A pane this size lays out its own header, body and sidebar. The modal only carries the
					surface, the backdrop and the placement.
				</span>
			</div>
		),
	},
});
