import preview from "#storybook/preview";
import { Toast, type ToastManagerAddOptions } from "@base-ui/react";
import type { FC } from "react";
import { Button } from "./Button.tsx";
import { Toasts, type ToastData, type ToastType } from "./Toasts.tsx";

type Options = ToastManagerAddOptions<ToastData> & { type?: ToastType };

// A toast added before the provider subscribes is dropped, so each story raises its toasts from a
// button and the play function presses it.
const Trigger: FC<{ toasts: Array<Options> }> = ({ toasts }) => {
	const toastManager = Toast.useToastManager();
	return (
		// Stays up until dismissed, so a snapshot finds it.
		<Button onClick={() => toasts.forEach((toast) => toastManager.add({ timeout: 0, ...toast }))}>
			Show
		</Button>
	);
};

const figma = (nodeId: string) => ({
	design: {
		type: "figma",
		url: `https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=${nodeId}`,
	},
});

const meta = preview.meta({
	title: "Overlays/Toasts",
	id: "components-toasts",
	component: Toasts,
	play: async ({ canvas, userEvent }) => {
		await userEvent.click(canvas.getByRole("button", { name: "Show" }));
	},
});

const Raise: FC<{ toasts: Array<Options> }> = ({ toasts }) => (
	<Toast.Provider>
		<Trigger toasts={toasts} />
		<Toasts />
	</Toast.Provider>
);

/**
 * A failure in the window's bottom-right corner: the title says what failed, and the raw error waits
 * behind "Copy error message" for a bug report instead of showing under it.
 */
export const Default = meta.story({
	parameters: figma("2805-61892"),
	render: () => (
		<Raise
			toasts={[
				{
					type: "error",
					title: "Failed to create comment",
					actionProps: { children: "Copy error message" },
					priority: "high",
				},
			]}
		/>
	),
});

/** A failure with a way round it: the action sits under the description, however long its label. */
export const ApplyConflict = meta.story({
	parameters: figma("2811-1414"),
	render: () => (
		<Raise
			toasts={[
				{
					type: "error",
					title: "Failed to apply branch",
					description:
						"'feature/login' conflicts with existing stack in the workspace: auth-refactor",
					priority: "high",
					actionProps: { children: "Switch to branch instead" },
				},
			]}
		/>
	),
});

/** A commit that came off in part: the description breaks down what was left behind, and why. */
export const RejectedChanges = meta.story({
	parameters: figma("2811-1438"),
	render: () => (
		<Raise
			toasts={[
				{
					type: "warning",
					title: "Some changes were not committed",
					description: (
						<ul>
							<li>
								<strong>Workspace merge conflict:</strong> src/app.ts and src/lib/api.ts
							</li>
							<li>
								<strong>File too large or binary:</strong> assets/logo.psd
							</li>
						</ul>
					),
					priority: "high",
				},
			]}
		/>
	),
});

/** News with no verdict, and the next step it offers; `data.icon` gives it the updater's own glyph. */
export const UpdateReady = meta.story({
	parameters: figma("2811-1462"),
	render: () => (
		<Raise
			toasts={[
				{
					type: "info",
					title: "Check for updates",
					description: "Update downloaded. Restart now or install on quit.",
					actionProps: { children: "Install 0.18.2 now" },
					data: { icon: "arrow-in-box" },
				},
			]}
		/>
	),
});

/** A success the screen doesn't otherwise show. */
export const ConflictsResolved = meta.story({
	parameters: figma("2811-1486"),
	render: () => (
		<Raise
			toasts={[
				{
					type: "success",
					title: "3 conflicts resolved",
					description: "2 conflicts remaining in this commit.",
					priority: "low",
				},
			]}
		/>
	),
});

const stack: Array<Options> = [
	{
		type: "error",
		title: "Failed to create comment",
		actionProps: { children: "Copy error message" },
	},
	{
		type: "success",
		title: "3 conflicts resolved",
		description: "2 conflicts remaining in this commit.",
	},
	{
		type: "error",
		title: "Failed to apply branch",
		description: "'feature/login' conflicts with existing stack in the workspace: auth-refactor",
		actionProps: { children: "Switch to branch instead" },
	},
];

/** Several at once fold behind the newest: the ones further back show only their top edge. */
export const Stack = meta.story({
	parameters: figma("2821-63766"),
	render: () => <Raise toasts={stack} />,
});

/** Under the pointer or keyboard focus the stack unfolds into a column, 8px apart. */
export const StackExpanded = meta.story({
	parameters: figma("2821-63761"),
	render: () => <Raise toasts={stack} />,
	play: async ({ canvas, userEvent, canvasElement }) => {
		await userEvent.click(canvas.getByRole("button", { name: "Show" }));
		// The toasts render in a portal outside the canvas, a frame or two after the click.
		const doc = canvasElement.ownerDocument;
		let front = doc.querySelector('[aria-label="Dismiss"]');
		for (let tries = 0; front === null && tries < 50; tries++) {
			await new Promise((resolve) => setTimeout(resolve, 20));
			front = doc.querySelector('[aria-label="Dismiss"]');
		}
		if (front === null) throw new Error("No toast appeared");
		await userEvent.hover(front);
	},
});
