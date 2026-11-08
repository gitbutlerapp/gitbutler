import { stories } from "../docs/stories.ts";
import { Button, type ButtonProps } from "./Button.tsx";
import { Icon } from "./Icon.tsx";
import { Kbd } from "./Kbd.tsx";
import React from "react";

const meta = stories({
	title: "Actions/Button",
	id: "components-button",
	component: Button,
});

const invertedDemoStyle: React.CSSProperties = {
	display: "inline-flex",
	padding: 8,
	borderRadius: 8,
	backgroundColor: "var(--clr-gray-10)",
};

export const Default = meta.story({
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=292-232&t=Sw2wSDYXtPlZ9Cao-1",
		},
	},
	args: {
		children: "Button",
		variant: "pop",
		size: "regular",
		showIcon: false,
	},
	render: (args: ButtonProps & { showIcon?: boolean }) => {
		const { showIcon, children, ...buttonArgs } = args;

		const button = (
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

		if (args.variant === "ghost-inverted") return <div style={invertedDemoStyle}>{button}</div>;

		return button;
	},
});

export const Variants = meta.story({
	render: () => (
		<div style={{ display: "grid", gridTemplateColumns: "repeat(7, max-content)", gap: 12 }}>
			<Button variant="pop">Button</Button>
			<Button variant="gray">Button</Button>
			<Button variant="outline">Button</Button>
			<div style={invertedDemoStyle}>
				<Button variant="outline-inverted">Button</Button>
			</div>
			<Button variant="ghost">Button</Button>
			<div style={invertedDemoStyle}>
				<Button variant="ghost-inverted">Button</Button>
			</div>
			<Button variant="danger">Button</Button>
		</div>
	),
});

export const IconOnly = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 12 }}>
			<Button variant="pop" iconOnly aria-label="Pop action">
				<Icon name="plus" />
			</Button>
			<Button variant="gray" iconOnly aria-label="Gray action">
				<Icon name="plus" />
			</Button>
			<Button variant="outline" iconOnly aria-label="Outline action">
				<Icon name="plus" />
			</Button>
			<div style={invertedDemoStyle}>
				<Button variant="outline-inverted" iconOnly aria-label="Outline inverted action">
					<Icon name="plus" />
				</Button>
			</div>
			<Button variant="ghost" iconOnly aria-label="Ghost action">
				<Icon name="plus" />
			</Button>
			<div style={invertedDemoStyle}>
				<Button variant="ghost-inverted" iconOnly aria-label="Ghost inverted action">
					<Icon name="plus" />
				</Button>
			</div>
			<Button variant="danger" iconOnly aria-label="Danger action">
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

const kbdExamples = [
	{ variant: "pop", label: "Commit", hotkey: "Mod+Enter" },
	{ variant: "gray", label: "Undo", hotkey: "Mod+Z" },
	{ variant: "outline", label: "Toggle files", hotkey: "F" },
	{ variant: "ghost", label: "Amend", hotkey: "Shift+A" },
	{ variant: "danger", label: "Delete", hotkey: "Mod+Backspace" },
] as const;

export const WithKbd = meta.story({
	render: () => (
		<div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
			{(["regular", "small"] as const).map((size) => (
				<div key={size} style={{ display: "flex", gap: 12, alignItems: "center" }}>
					{kbdExamples.map(({ variant, label, hotkey }) => (
						<Button key={variant} variant={variant} size={size}>
							{label}
							<Kbd hotkey={hotkey} variant="button" />
						</Button>
					))}
				</div>
			))}
		</div>
	),
});
