import preview from "#storybook/preview";
import { expect, fn, userEvent, within } from "storybook/test";

import { DropdownButton } from "./DropdownButton.tsx";

const meta = preview.meta({
	component: DropdownButton,
});

export const Playground = meta.story({
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=333-415&t=ZysFmCVuSbtY5q6A-1",
		},
	},
	args: {
		children: "Commit",
		variant: "pop",
		size: "regular",
		disabled: false,
		menuAriaLabel: "More options",
		onClick: fn(),
		onMenuOpen: fn(),
	},
	argTypes: {
		variant: {
			control: "select",
			options: ["pop", "gray", "outline", "danger"],
		},
		size: {
			control: "radio",
			options: ["regular", "small"],
		},
	},
	play: async ({ canvasElement, args }) => {
		const canvas = within(canvasElement);

		await userEvent.click(canvas.getByRole("button", { name: "Commit" }));
		await userEvent.click(canvas.getByRole("button", { name: "More options" }));

		await expect(args.onClick).toHaveBeenCalledTimes(1);
		await expect(args.onMenuOpen).toHaveBeenCalledTimes(1);
	},
});

export const Variants = meta.story({
	render: () => (
		<div style={{ display: "grid", gridTemplateColumns: "repeat(6, max-content)", gap: 12 }}>
			{(["pop", "gray", "outline", "danger"] as const).map((variant) => (
				<DropdownButton key={variant} variant={variant} onMenuOpen={fn()}>
					{variant}
				</DropdownButton>
			))}
		</div>
	),
});

export const DisabledStates = meta.story({
	render: () => (
		<div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
			<DropdownButton onMenuOpen={fn()}>Enabled</DropdownButton>
			<DropdownButton disabled onMenuOpen={fn()}>
				Disabled
			</DropdownButton>
		</div>
	),
});

export const DisabledInteraction = meta.story({
	args: {
		children: "Commit",
		disabled: true,
		menuAriaLabel: "More options",
		onClick: fn(),
		onMenuOpen: fn(),
	},
	play: async ({ canvasElement, args }) => {
		const canvas = within(canvasElement);

		await userEvent.click(canvas.getByRole("button", { name: "Commit" }));
		await userEvent.click(canvas.getByRole("button", { name: "More options" }));

		await expect(args.onClick).toHaveBeenCalledTimes(0);
		await expect(args.onMenuOpen).toHaveBeenCalledTimes(0);
	},
});
