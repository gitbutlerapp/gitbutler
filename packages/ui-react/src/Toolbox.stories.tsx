import preview from "#storybook/preview";
import { getButtonClassName } from "./Button.tsx";
import { Icon } from "./Icon.tsx";
import { Kbd } from "./Kbd.tsx";
import {
	Toolbox,
	ToolboxMeta,
	ToolboxMetaHint,
	ToolboxMetaText,
	ToolboxSection,
	ToolboxSeparator,
	ToolboxStack,
} from "./Toolbox.tsx";
import { ToggleGroupStyles, ToggleStyles } from "./ToggleGroup.tsx";
import { ToggleGroup, Toggle } from "@base-ui/react";
import type { ButtonVariant } from "./Button.tsx";
import type { FC } from "react";

const meta = preview.meta({
	component: Toolbox,
	parameters: {
		// The toolbox floats over a workspace; previewing it flush against the canvas edge clips
		// the border and shadow that give it its lift.
		layout: "centered",
		design: {
			type: "figma",
			url: "https://www.figma.com/design/EBuHQGUcCaSw4Ln5uVpWkn/Lite?node-id=4272-2053",
		},
	},
});

const Action: FC<{ label: string; hotkey?: string; variant?: ButtonVariant; small?: boolean }> = ({
	label,
	hotkey,
	variant,
	small,
}) => (
	<button
		type="button"
		className={getButtonClassName({ variant, size: small ? "small" : "regular" })}
	>
		{label}
		{hotkey !== undefined && <Kbd hotkey={hotkey} variant="button" />}
	</button>
);

/**
 * The acts on a checked set run without confirming, so each one is its own button. With nothing
 * pending to abandon, the way out is a close affordance and its chord is stated in the strip.
 */
export const Actions = meta.story({
	render: () => (
		<Toolbox>
			<ToolboxMeta icon="commit">
				<span>3 commits selected</span>
				<ToolboxMetaHint>Esc to close</ToolboxMetaHint>
			</ToolboxMeta>
			<ToolboxSection>
				<Action label="Absorb" hotkey="A" />
				<Action label="Cut" hotkey="Mod+X" />
				<Action label="Discard" hotkey="Mod+Backspace" variant="danger" />
				<ToolboxSeparator />
				<button
					type="button"
					aria-label="Cancel"
					className={getButtonClassName({ variant: "ghost", iconOnly: true })}
				>
					<Icon name="cross" />
				</button>
			</ToolboxSection>
		</Toolbox>
	),
});

/** An operation still being aimed asks for a placement, then for confirmation. */
export const WithConfirmation = meta.story({
	render: () => (
		<Toolbox style={{ width: 270 }}>
			<ToolboxMeta icon="file-diff">
				<span>3 files selected</span>
			</ToolboxMeta>
			<ToolboxSection variant="stretch">
				<ToggleGroup render={<ToggleGroupStyles />} defaultValue={["above"]} aria-label="Placement">
					<Toggle render={<ToggleStyles />} value="above">
						Above <Kbd hotkey="A" variant="button" />
					</Toggle>
					<Toggle render={<ToggleStyles />} value="below">
						Below <Kbd hotkey="B" variant="button" />
					</Toggle>
					<Toggle render={<ToggleStyles />} value="into">
						Into <Kbd hotkey="I" variant="button" />
					</Toggle>
				</ToggleGroup>
			</ToolboxSection>
			<ToolboxSection variant="confirm">
				<Action label="Move commit" hotkey="Mod+Enter" variant="gray" small />
				<Action label="Cancel" hotkey="Escape" small />
			</ToolboxSection>
		</Toolbox>
	),
});

/**
 * A transfer that can be copied picks its kind in an addon above, and states its subject as a
 * sentence.
 */
export const WithTypeSelector = meta.story({
	render: () => (
		<ToolboxStack style={{ width: 322 }}>
			<Toolbox>
				<ToolboxSection variant="stretch">
					<ToggleGroup render={<ToggleGroupStyles />} defaultValue={["move"]} aria-label="Kind">
						<Toggle render={<ToggleStyles />} value="move">
							Move <Kbd hotkey="M" variant="button" />
						</Toggle>
						<Toggle render={<ToggleStyles />} value="copy">
							Copy <Kbd hotkey="C" variant="button" />
						</Toggle>
					</ToggleGroup>
				</ToolboxSection>
			</Toolbox>
			<Toolbox>
				<ToolboxMeta icon="commit">
					<strong>Move</strong>
					<ToolboxMetaText>docs: update endpoint documentation</ToolboxMetaText>
					<strong>below</strong>
					<ToolboxMetaText>migrate BaseButton to TypeScript</ToolboxMetaText>
				</ToolboxMeta>
				<ToolboxSection variant="stretch">
					<ToggleGroup
						render={<ToggleGroupStyles />}
						defaultValue={["above"]}
						aria-label="Placement"
					>
						<Toggle render={<ToggleStyles />} value="above">
							Above <Kbd hotkey="A" variant="button" />
						</Toggle>
						<Toggle render={<ToggleStyles />} value="below">
							Below <Kbd hotkey="B" variant="button" />
						</Toggle>
						<Toggle render={<ToggleStyles />} value="into">
							Into <Kbd hotkey="I" variant="button" />
						</Toggle>
					</ToggleGroup>
				</ToolboxSection>
				<ToolboxSection variant="confirm">
					<Action label="Cherry-pick" hotkey="Mod+Enter" variant="gray" small />
					<Action label="Cancel" hotkey="Escape" small />
				</ToolboxSection>
			</Toolbox>
		</ToolboxStack>
	),
});

/** Nothing but the choice, for a toolbox that has already stated its subject elsewhere. */
export const TypeAddon = meta.story({
	render: () => (
		<Toolbox style={{ width: 322 }}>
			<ToolboxSection variant="stretch">
				<ToggleGroup render={<ToggleGroupStyles />} defaultValue={["move"]} aria-label="Kind">
					<Toggle render={<ToggleStyles />} value="move">
						Move <Kbd hotkey="M" variant="button" />
					</Toggle>
					<Toggle render={<ToggleStyles />} value="copy">
						Copy <Kbd hotkey="C" variant="button" />
					</Toggle>
				</ToggleGroup>
			</ToolboxSection>
		</Toolbox>
	),
});
