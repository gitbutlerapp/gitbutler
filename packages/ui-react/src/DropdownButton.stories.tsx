import preview from "#storybook/preview";
import { DropdownButton, type DropdownButtonVariant } from "./DropdownButton.tsx";
import { Kbd } from "./Kbd.tsx";
import { Tooltip } from "@base-ui/react";
import { useState } from "react";

const meta = preview.meta({
	component: DropdownButton,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=333-415&t=WybubQHtVH7UitJU-1",
		},
	},
	decorators: [
		(Story) => (
			<Tooltip.Provider>
				<Story />
			</Tooltip.Provider>
		),
	],
});

const variants = ["pop", "gray", "outline"] as const satisfies ReadonlyArray<DropdownButtonVariant>;

/** Stands in for the native menu, which Storybook has no host process for. */
const Demo = ({
	label = "Button",
	variant,
	disabled,
	menuDisabled,
}: {
	label?: string;
	variant?: DropdownButtonVariant;
	disabled?: boolean;
	menuDisabled?: boolean;
}) => {
	const [lastAction, setLastAction] = useState<string | null>(null);

	return (
		<div style={{ display: "flex", gap: 12, alignItems: "center" }}>
			<DropdownButton
				variant={variant}
				disabled={disabled}
				menuDisabled={menuDisabled}
				menuLabel={`${label} options`}
				onClick={() => setLastAction("clicked the action")}
				onMenuTrigger={(trigger) => setLastAction(`opened a menu on ${trigger.tagName}`)}
			>
				{label}
			</DropdownButton>
			<span className="text-12">{lastAction ?? " "}</span>
		</div>
	);
};

export const Playground = meta.story({
	argTypes: {
		variant: { control: "radio", options: variants },
		size: { control: "radio", options: ["regular", "small"] },
		children: { control: "text" },
		disabled: { control: "boolean" },
		menuDisabled: { control: "boolean" },
	},
	args: {
		children: "Button",
		menuLabel: "Button options",
		variant: "pop",
		size: "regular",
		disabled: false,
		menuDisabled: false,
		onMenuTrigger: () => {},
	},
});

/** Pop carries the primary action; gray and outline sit lower in the hierarchy. */
export const Variants = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 12, flexDirection: "column", alignItems: "flex-start" }}>
			{variants.map((variant) => (
				<Demo key={variant} variant={variant} />
			))}
		</div>
	),
});

/** The halves disable independently: an unavailable action can still offer its menu. */
export const Disabled = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 12, flexDirection: "column", alignItems: "flex-start" }}>
			<Demo variant="pop" disabled />
			<Demo variant="pop" menuDisabled />
			<Demo variant="outline" disabled menuDisabled />
		</div>
	),
});

/** Small matches the surrounding row when the button sits inside dense chrome. */
export const Sizes = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 12, alignItems: "center" }}>
			<DropdownButton
				variant="pop"
				size="small"
				menuLabel="Commit options"
				onMenuTrigger={() => {}}
			>
				Commit
			</DropdownButton>
			<DropdownButton variant="pop" menuLabel="Commit options" onMenuTrigger={() => {}}>
				Commit
			</DropdownButton>
		</div>
	),
});

/**
 * The action fills the space it's given and ellipsises, while the chevron keeps
 * its width — as in the sidebar, where the button spans the panel.
 */
export const Stretched = meta.story({
	render: () => (
		// A column flex container stretches its item across, as the sidebar does.
		<div style={{ display: "flex", flexDirection: "column", width: 220 }}>
			<DropdownButton variant="pop" menuLabel="Commit options" onMenuTrigger={() => {}}>
				Start commit
				<Kbd hotkey="Mod+Enter" variant="button" />
			</DropdownButton>
		</div>
	),
});

/**
 * A disabled action can still say why: both halves stay hoverable while
 * disabled, so the tooltip needs no wrapper element. This is the pull request
 * pane's merge button, blocked on its checks.
 */
export const BlockedAction = meta.story({
	render: () => (
		<DropdownButton
			variant="pop"
			disabled
			actionTooltip="Blocked: required approvals or checks are not satisfied"
			menuLabel="Merge method"
			onMenuTrigger={() => {}}
		>
			Merge
		</DropdownButton>
	),
});
