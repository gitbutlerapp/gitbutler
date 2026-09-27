import preview from "#storybook/preview";
import { Button } from "./Button.tsx";
import { FieldControlStyles, FieldRootStyles } from "./Field.tsx";
import { FileIcon } from "./FileIcon.tsx";
import { List, ListItem } from "./List.tsx";
import { Modal, ModalBody, ModalFooter, ModalHeader } from "./Popup.tsx";
import { Field } from "@base-ui/react";
import { useState } from "react";

const figma = (nodeId: string) => ({
	design: {
		type: "figma",
		url: `https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=${nodeId}`,
	},
});

const meta = preview.meta({
	component: Modal,
	parameters: figma("2229-1566"),
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
 * The modal and its props, to try with the controls. The stories after it are modals as the app
 * writes them — a confirmation, a form, a destructive question — opened and whole, to copy from.
 */
export const Default = meta.story({
	args: {
		children: (
			<>
				<ModalHeader
					title="Leave without saving?"
					description="Your edits to the description are lost."
				/>
				<ModalFooter>
					<Button variant="ghost">Cancel</Button>
					<Button variant="gray">Leave</Button>
				</ModalFooter>
			</>
		),
	},
});

/**
 * A confirmation: the title asks, the description says what answering does, the body shows what
 * it acts on, and the footer holds Cancel and the answer. The answer is `gray` and repeats the
 * title's verb. Escape and the backdrop cancel it.
 */
export const Confirm = meta.story({
	parameters: figma("2283-1871"),
	render: function Confirm() {
		const [open, setOpen] = useState(true);
		return (
			<Modal
				size="small"
				open={open}
				onOpenChange={setOpen}
				trigger={<Button>Upload files</Button>}
			>
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
					<Button variant="ghost" onClick={() => setOpen(false)}>
						Cancel
					</Button>
					<Button variant="gray" onClick={() => setOpen(false)}>
						Upload
					</Button>
				</ModalFooter>
			</Modal>
		);
	},
});

/**
 * A short form. The `<form>` wraps all three parts so Enter submits, and the submit button is the
 * answer. The description names the one field, so it has no label above it, only an `aria-label`.
 */
export const Form = meta.story({
	parameters: figma("2283-1878"),
	render: function Form() {
		const [open, setOpen] = useState(true);
		return (
			<Modal
				alert
				size="small"
				open={open}
				onOpenChange={setOpen}
				trigger={<Button>Sign in to git</Button>}
			>
				<form
					onSubmit={(event) => {
						event.preventDefault();
						setOpen(false);
					}}
				>
					<ModalHeader
						title="Git credentials required"
						description="Enter your password for github.com to fetch."
					/>
					<ModalBody>
						<Field.Root render={<FieldRootStyles />}>
							<Field.Control
								render={<FieldControlStyles />}
								type="password"
								aria-label="Password"
							/>
						</Field.Root>
					</ModalBody>
					<ModalFooter>
						<Button variant="ghost" onClick={() => setOpen(false)}>
							Cancel
						</Button>
						<Button type="submit" variant="gray">
							Continue
						</Button>
					</ModalFooter>
				</form>
			</Modal>
		);
	},
});

/**
 * Something that cannot be taken back: the answer is `danger`, chosen by consequence. A one-line
 * question with nothing to list takes `xsmall` and no body.
 */
export const Destructive = meta.story({
	parameters: figma("2283-1882"),
	render: function Destructive() {
		const [open, setOpen] = useState(true);
		return (
			<Modal
				alert
				size="xsmall"
				open={open}
				onOpenChange={setOpen}
				trigger={<Button>Discard changes</Button>}
			>
				<ModalHeader
					title="Discard 3 files?"
					description="Their changes are gone for good; nothing keeps a copy."
				/>
				<ModalFooter>
					<Button variant="ghost" onClick={() => setOpen(false)}>
						Cancel
					</Button>
					<Button variant="danger" onClick={() => setOpen(false)}>
						Discard
					</Button>
				</ModalFooter>
			</Modal>
		);
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
