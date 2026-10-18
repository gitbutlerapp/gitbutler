import preview from "#storybook/preview";
import { useState } from "react";
import { Icon } from "./Icon.tsx";
import { Segment, SegmentedControl } from "./SegmentedControl.tsx";

const figma = (nodeId: string) => ({
	design: {
		type: "figma" as const,
		url: `https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=${nodeId}`,
	},
});

const meta = preview.meta({
	title: "Actions/SegmentedControl",
	id: "components-segmentedcontrol",
	component: SegmentedControl,
	parameters: figma("3485-80297"),
});

/** Two ways to show a diff; one is always picked. */
export const Default = meta.story({
	parameters: figma("3485-80320"),
	render: () => (
		<SegmentedControl defaultValue="split" aria-label="Diff layout">
			<Segment value="split">Split</Segment>
			<Segment value="unified">Unified</Segment>
		</SegmentedControl>
	),
});

/** A setting with three values, held by the app. */
export const Theme = meta.story({
	parameters: figma("3485-80361"),
	render: function Theme() {
		const [theme, setTheme] = useState<"system" | "light" | "dark">("system");
		return (
			<SegmentedControl value={theme} onValueChange={setTheme} aria-label="Theme">
				<Segment value="system" icon={<Icon name="theme-system" />}>
					System
				</Segment>
				<Segment value="light" icon={<Icon name="theme-light" />}>
					Light
				</Segment>
				<Segment value="dark" icon={<Icon name="theme-dark" />}>
					Dark
				</Segment>
			</SegmentedControl>
		);
	},
});

/** An option that isn't available for this branch stays visible at half strength. */
export const Unavailable = meta.story({
	parameters: figma("3485-80410"),
	render: () => (
		<SegmentedControl defaultValue="mine" aria-label="Keep changes">
			<Segment value="mine">Mine</Segment>
			<Segment value="combine" disabled>
				Combine
			</Segment>
			<Segment value="theirs">Theirs</Segment>
		</SegmentedControl>
	),
});

/** Small, in a popup's header, with how many unseen notifications each side holds. */
export const Notifications = meta.story({
	parameters: figma("3485-80458"),
	render: () => (
		<SegmentedControl defaultValue="humans" size="small" aria-label="Notification type">
			<Segment value="humans" count={2}>
				Humans
			</Segment>
			<Segment value="agents" count={5}>
				Agents
			</Segment>
		</SegmentedControl>
	),
});

/** Small, over a chart: the range it covers. */
export const TimeRange = meta.story({
	parameters: figma("3485-80519"),
	render: () => (
		<SegmentedControl defaultValue="1h" size="small" aria-label="Time range">
			<Segment value="1h">1h</Segment>
			<Segment value="24h">24h</Segment>
			<Segment value="7d">7d</Segment>
		</SegmentedControl>
	),
});

// Waits for the raised ground to sit under the picked segment, as it slides there.
const groundSettles = async (track: HTMLElement, step: string) => {
	const offBy = () => {
		const picked = track.querySelector("[data-pressed]");
		if (picked === null) return Infinity;
		const segment = picked.getBoundingClientRect();
		const ground = getComputedStyle(track, "::before");
		return Math.max(
			Math.abs(
				Number.parseFloat(ground.translate) - (segment.left - track.getBoundingClientRect().left),
			),
			Math.abs(Number.parseFloat(ground.width) - segment.width),
		);
	};
	for (let tries = 0; offBy() > 0.5 && tries < 50; tries++)
		await new Promise((resolve) => setTimeout(resolve, 20));
	if (offBy() > 0.5) throw new Error(`${step}: the ground is ${offBy()}px off the picked segment`);
};

/** Each placement with the key that picks it, as the operation controls show them. */
export const WithShortcuts = meta.story({
	parameters: figma("3490-1001"),
	play: async ({ canvas, userEvent }) => {
		const track = canvas.getByRole("group", { name: "Placement" });
		await groundSettles(track, "At first");
		await userEvent.click(canvas.getByRole("button", { name: /Below/ }));
		await groundSettles(track, "Picking Below");
		// The segments grow without the pick changing, as a font loading or a count arriving does.
		track.style.setProperty("--segment-padding-inline", "16px");
		await groundSettles(track, "As the segments grow");
		track.style.removeProperty("--segment-padding-inline");
	},
	render: () => (
		<SegmentedControl defaultValue="into" aria-label="Placement">
			<Segment value="above" kbd="Mod+ArrowUp">
				Above
			</Segment>
			<Segment value="into" kbd="Mod+Enter">
				Into
			</Segment>
			<Segment value="below" kbd="Mod+ArrowDown">
				Below
			</Segment>
		</SegmentedControl>
	),
});
