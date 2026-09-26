import preview from "#storybook/preview";
import { Toast } from "@base-ui/react";
import type { FC } from "react";
import { Button } from "./Button.tsx";
import { Toasts } from "./Toasts.tsx";

const Triggers: FC = () => {
	const toastManager = Toast.useToastManager();
	return (
		<div style={{ display: "flex", gap: 8 }}>
			<Button
				onClick={() =>
					toastManager.add({
						title: "Failed to create comment",
						description: "The forge refused the request: 403 Forbidden.",
						priority: "high",
						// Stays up until dismissed, so a snapshot finds it.
						timeout: 0,
					})
				}
			>
				Show a toast
			</Button>
			<Button
				onClick={() =>
					toastManager.add({
						title: "Branch unapplied",
						description: "Its changes are kept, and it can be applied again.",
						actionProps: { children: "Undo" },
						timeout: 0,
					})
				}
			>
				Show a toast with an action
			</Button>
		</div>
	);
};

const meta = preview.meta({
	component: Toasts,
	render: () => (
		<Toast.Provider>
			<Triggers />
			<Toasts />
		</Toast.Provider>
	),
});

/** A title, a description, and Dismiss, in the window's bottom-right corner. */
export const Default = meta.story({
	play: async ({ canvas, userEvent }) => {
		await userEvent.click(canvas.getByRole("button", { name: "Show a toast" }));
	},
});

/** With `actionProps`, the toast offers its action beside Dismiss. */
export const WithAction = meta.story({
	play: async ({ canvas, userEvent }) => {
		await userEvent.click(canvas.getByRole("button", { name: "Show a toast with an action" }));
	},
});
