import preview from "#storybook/preview";
import { Button } from "./Button.tsx";
import { FieldControlStyles, FieldRootStyles } from "./Field.tsx";
import { FileIcon } from "./FileIcon.tsx";
import { List, ListItem } from "./List.tsx";
import {
	Modal,
	ModalBody,
	ModalFooter,
	ModalHeader,
	PopupItem,
	PopupSearch,
	PopupSection,
} from "./Popup.tsx";
import { Field } from "@base-ui/react";

const meta = preview.meta({
	component: Modal,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=2229-1566",
		},
	},
	args: {
		size: "small",
		align: "center",
		alert: false,
		trigger: <Button>Open modal</Button>,
	},
	argTypes: {
		size: { control: "inline-radio", options: ["xsmall", "small", "medium", "large"] },
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

/**
 * A confirmation, built from the three parts: a title and what answering does, what it acts on,
 * then Cancel and the answer. The answer is `gray`, not `pop` — two buttons are not the busy
 * surface pop is kept for.
 */
export const Playground = meta.story({
	args: {
		children: (
			<>
				<ModalHeader
					title="Upload these 2 files?"
					description="They go to gitbutler.com, and anyone with the link can open them."
				/>
				<ModalBody>
					<List>
						<ListItem marker={<FileIcon fileName="screenshot.png" />}>screenshot.png</ListItem>
						<ListItem marker={<FileIcon fileName="pasted-image.png" />}>pasted-image.png</ListItem>
					</List>
				</ModalBody>
				<ModalFooter>
					<Button variant="ghost">Cancel</Button>
					<Button variant="gray">Upload</Button>
				</ModalFooter>
			</>
		),
	},
});

/**
 * A short form. The `<form>` wraps all three parts so Enter submits, and the modal lays it out as
 * it would the parts on their own. The description names the one field, so it has no label above
 * it, only an `aria-label`.
 */
export const Form = meta.story({
	args: {
		alert: true,
		trigger: <Button>Sign in to git</Button>,
		children: (
			<form onSubmit={(event) => event.preventDefault()}>
				<ModalHeader
					title="Git credentials required"
					description="Enter your password for github.com to fetch."
				/>
				<ModalBody>
					<Field.Root render={<FieldRootStyles />}>
						<Field.Control render={<FieldControlStyles />} type="password" aria-label="Password" />
					</Field.Root>
				</ModalBody>
				<ModalFooter>
					<Button variant="ghost">Cancel</Button>
					<Button type="submit" variant="gray">
						Continue
					</Button>
				</ModalFooter>
			</form>
		),
	},
});

/** Something that cannot be taken back: the answer is `danger`, chosen by consequence. A one-line question takes the `xsmall` width. */
export const Destructive = meta.story({
	args: {
		size: "xsmall",
		alert: true,
		trigger: <Button>Discard changes</Button>,
		children: (
			<>
				<ModalHeader
					title="Discard 3 files?"
					description="Their changes are gone for good; nothing keeps a copy."
				/>
				<ModalFooter>
					<Button variant="ghost">Cancel</Button>
					<Button variant="danger">Discard</Button>
				</ModalFooter>
			</>
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
