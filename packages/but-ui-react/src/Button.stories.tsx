import { stories } from "../stories/stories.ts";
import { Button, type ButtonProps } from "./Button.tsx";
import { Icon } from "./Icon.tsx";
import { Kbd } from "./Kbd.tsx";
import { Segment, SegmentedControl } from "./SegmentedControl.tsx";
import { ToggleGroupStyles, ToggleStyles } from "./ToggleGroup.tsx";
import { Toggle, ToggleGroup } from "@base-ui/react";
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
	backgroundColor: "var(--but-inverse)",
};

export const Default = meta.story({
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=217-317",
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

/**
 * The coloured outlines, each a count's next step in a row: Push beside the unpushed count in blue,
 * Update beside what's behind in amber, Merge once checks pass in green. The number and the count's
 * glyph say what the step acts on.
 */
export const Colours = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 12, alignItems: "center" }}>
			<Button size="small" variant="info" aria-label="Push 8 commits">
				8
				<Icon name="arrow-up" />
			</Button>
			<Button size="small" variant="warning" aria-label="Update 3 commits">
				3
				<Icon name="arrow-down" />
			</Button>
			<Button size="small" variant="success">
				Merge
			</Button>
			<Button variant="info">Push</Button>
			<Button variant="warning">Update</Button>
			<Button variant="success">Merge</Button>
		</div>
	),
});

/**
 * Danger at both strengths: soft for a destructive act in a row or a toolbar, and `danger-solid`
 * for the answer to a confirmation that destroys something.
 */
export const Danger = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 12 }}>
			<Button variant="danger">Discard</Button>
			<Button variant="danger-solid">Discard</Button>
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
			{(["regular", "medium", "small"] as const).map((size) => (
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

/**
 * A dense header's controls, all 24px: a small `SegmentedControl`, a `ToggleGroup` of medium
 * toggles and medium buttons, so the row reads as one line.
 */
export const DenseHeader = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 8, alignItems: "center" }}>
			<SegmentedControl size="small" aria-label="Range" defaultValue="24h">
				<Segment value="1h">1h</Segment>
				<Segment value="24h">24h</Segment>
				<Segment value="7d">7d</Segment>
			</SegmentedControl>
			<ToggleGroup render={<ToggleGroupStyles />} defaultValue={["humans"]} aria-label="Show">
				<Toggle render={<ToggleStyles size="medium" />} value="humans">
					Humans
				</Toggle>
				<Toggle render={<ToggleStyles size="medium" />} value="agents">
					Agents
				</Toggle>
			</ToggleGroup>
			<Button variant="outline" size="medium">
				<Icon name="plus" />
				New branch
			</Button>
			<Button variant="ghost" size="medium" iconOnly aria-label="More">
				<Icon name="kebab" />
			</Button>
		</div>
	),
});
