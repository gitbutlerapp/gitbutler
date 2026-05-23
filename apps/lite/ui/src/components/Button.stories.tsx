import preview from "#storybook/preview";
import { Icon } from "#ui/components/Icon.tsx";
import { expect, fn, userEvent, within } from "storybook/test";

import { Button } from "./Button.tsx";

const meta = preview.meta({
	component: Button,
});

export const Playground = meta.story({
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=292-232&t=Sw2wSDYXtPlZ9Cao-1",
		},
	},
	argTypes: {
		variant: {
			control: "select",
			options: ["pop", "gray", "outline", "danger", "ghost", "inverted"],
		},
		size: {
			control: "radio",
			options: ["regular", "small"],
		},
		showIcon: {
			control: "boolean",
		},
	},
	args: {
		children: "Button",
		variant: "pop",
		size: "regular",
		showIcon: false,
		onClick: fn(),
	},
	render: (args: React.ComponentProps<typeof Button> & { showIcon?: boolean }) => {
		const { showIcon, children, ...buttonArgs } = args;

		return (
			<Button {...buttonArgs}>
				{showIcon ? (
					<>
						<Icon name="plus" />
						{children}
					</>
				) : (
					children
				)}
			</Button>
		);
	},
	play: async ({ canvasElement, args }) => {
		const canvas = within(canvasElement);
		await userEvent.click(canvas.getByRole("button"));
		await expect(args.onClick).toHaveBeenCalledTimes(1);
	},
});

export const Variants = meta.story({
	render: () => (
		<div style={{ display: "grid", gridTemplateColumns: "repeat(6, max-content)", gap: 12 }}>
			<Button variant="pop">Button</Button>
			<Button variant="gray">Button</Button>
			<Button variant="outline">Button</Button>
			<Button variant="ghost">Button</Button>
			<Button variant="danger">Button</Button>
			<Button variant="inverted">Button</Button>
		</div>
	),
});

export const IconOnly = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 12 }}>
			<Button aria-label="Pop action">
				<Icon name="plus" />
			</Button>
			<Button variant="gray" aria-label="Gray action">
				<Icon name="plus" />
			</Button>
			<Button variant="outline" aria-label="Outline action">
				<Icon name="plus" />
			</Button>
			<Button variant="ghost" aria-label="Ghost action">
				<Icon name="plus" />
			</Button>
			<Button variant="danger" aria-label="Danger action">
				<Icon name="plus" />
			</Button>
			<Button variant="inverted" aria-label="Inverted action">
				<Icon name="plus" />
			</Button>
		</div>
	),
});

export const WithIconStartAndEnd = meta.story({
	render: () => (
		<div style={{ display: "grid", gridTemplateColumns: "repeat(2, max-content)", gap: 12 }}>
			<Button variant="outline">
				<Icon name="branch" />
				New Branch
			</Button>
			<Button variant="outline">
				New Branch
				<Icon name="branch" />
			</Button>
		</div>
	),
});
