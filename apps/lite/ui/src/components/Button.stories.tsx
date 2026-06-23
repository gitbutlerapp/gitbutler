import preview from "#storybook/preview";
import {
	ButtonSize,
	ButtonVariant,
	getButtonClassName,
	type ButtonStyleProps,
} from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import React from "react";

const meta = preview.meta({});

const invertedDemoStyle: React.CSSProperties = {
	display: "inline-flex",
	padding: 8,
	borderRadius: 8,
	backgroundColor: "var(--clr-gray-10)",
};

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
			options: [
				"pop",
				"gray",
				"outline",
				"outline-inverted",
				"danger",
				"ghost",
				"ghost-inverted",
			] satisfies Array<ButtonVariant>,
		},
		size: {
			control: "radio",
			options: ["regular", "small"] satisfies Array<ButtonSize>,
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
	},
	render: (args: React.ComponentProps<"button"> & ButtonStyleProps & { showIcon?: boolean }) => {
		const { showIcon, children, variant, size, iconOnly, className, ...buttonArgs } = args;

		const button = (
			<button
				{...buttonArgs}
				type="button"
				className={classes(getButtonClassName({ variant, size, iconOnly }), className)}
			>
				{showIcon ? (
					<>
						<Icon name="plus" />
						{children}
					</>
				) : (
					children
				)}
			</button>
		);

		if (variant === "ghost-inverted") return <div style={invertedDemoStyle}>{button}</div>;

		return button;
	},
});

export const Variants = meta.story({
	render: () => (
		<div style={{ display: "grid", gridTemplateColumns: "repeat(7, max-content)", gap: 12 }}>
			<button type="button" className={getButtonClassName({ variant: "pop" })}>
				Button
			</button>
			<button type="button" className={getButtonClassName({ variant: "gray" })}>
				Button
			</button>
			<button type="button" className={getButtonClassName({ variant: "outline" })}>
				Button
			</button>
			<div style={invertedDemoStyle}>
				<button type="button" className={getButtonClassName({ variant: "outline-inverted" })}>
					Button
				</button>
			</div>
			<button type="button" className={getButtonClassName({ variant: "ghost" })}>
				Button
			</button>
			<div style={invertedDemoStyle}>
				<button type="button" className={getButtonClassName({ variant: "ghost-inverted" })}>
					Button
				</button>
			</div>
			<button type="button" className={getButtonClassName({ variant: "danger" })}>
				Button
			</button>
		</div>
	),
});

export const IconOnly = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 12 }}>
			<button
				type="button"
				className={getButtonClassName({ iconOnly: true })}
				aria-label="Pop action"
			>
				<Icon name="plus" />
			</button>
			<button
				type="button"
				className={getButtonClassName({ variant: "gray", iconOnly: true })}
				aria-label="Gray action"
			>
				<Icon name="plus" />
			</button>
			<button
				type="button"
				className={getButtonClassName({ variant: "outline", iconOnly: true })}
				aria-label="Outline action"
			>
				<Icon name="plus" />
			</button>
			<div style={invertedDemoStyle}>
				<button
					type="button"
					className={getButtonClassName({ variant: "outline-inverted", iconOnly: true })}
					aria-label="Outline inverted action"
				>
					<Icon name="plus" />
				</button>
			</div>
			<button
				type="button"
				className={getButtonClassName({ variant: "ghost", iconOnly: true })}
				aria-label="Ghost action"
			>
				<Icon name="plus" />
			</button>
			<div style={invertedDemoStyle}>
				<button
					type="button"
					className={getButtonClassName({ variant: "ghost-inverted", iconOnly: true })}
					aria-label="Ghost inverted action"
				>
					<Icon name="plus" />
				</button>
			</div>
			<button
				type="button"
				className={getButtonClassName({ variant: "danger", iconOnly: true })}
				aria-label="Danger action"
			>
				<Icon name="plus" />
			</button>
		</div>
	),
});

export const WithIconStartAndEnd = meta.story({
	render: () => (
		<div style={{ display: "grid", gridTemplateColumns: "repeat(2, max-content)", gap: 12 }}>
			<button type="button" className={getButtonClassName({ variant: "outline" })}>
				<Icon name="branch" />
				New Branch
			</button>
			<button type="button" className={getButtonClassName({ variant: "outline" })}>
				New Branch
				<Icon name="branch" />
			</button>
		</div>
	),
});
