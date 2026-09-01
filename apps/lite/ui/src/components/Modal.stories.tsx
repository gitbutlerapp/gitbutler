import preview from "#storybook/preview";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Modal, PopupItem, PopupSearch, PopupSection } from "./Popup.tsx";

const meta = preview.meta({
	component: Modal,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/EBuHQGUcCaSw4Ln5uVpWkn/Lite?node-id=4518-292100",
		},
	},
	args: {
		size: "small",
		align: "center",
		alert: false,
		trigger: (
			<button type="button" className={getButtonClassName({})}>
				Open modal
			</button>
		),
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
					<button type="button" className={getButtonClassName({ variant: "ghost" })}>
						Cancel
					</button>
					<button type="button" className={getButtonClassName({ variant: "pop" })}>
						Continue
					</button>
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
		trigger: (
			<button type="button" className={getButtonClassName({})}>
				Select project
			</button>
		),
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
		trigger: (
			<button type="button" className={getButtonClassName({})}>
				Resolve conflicts
			</button>
		),
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
