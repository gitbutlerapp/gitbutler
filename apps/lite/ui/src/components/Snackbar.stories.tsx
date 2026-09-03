import preview from "#storybook/preview";
import { Snackbar, type SnackbarVariant } from "./Snackbar.tsx";

const meta = preview.meta({
	component: Snackbar,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=1706-1682",
		},
	},
	argTypes: {
		variant: {
			control: "inline-radio",
			options: ["info", "danger", "safe"] satisfies Array<SnackbarVariant>,
		},
		icon: { control: "text" },
	},
	args: {
		variant: "info",
		children: "Info. Snackbar message",
		onDismiss: () => {},
	},
});

/** The snackbar as the design states it: a glyph, a sentence, and a way out. */
export const Default = meta.story({});

/** One surface, three glyphs: only the leading icon says how the news landed. */
export const AllVariants = meta.story({
	render: () => (
		<div style={{ display: "flex", flexDirection: "column", alignItems: "start", gap: 12 }}>
			<Snackbar>Info. Snackbar message</Snackbar>
			<Snackbar variant="danger">Danger. Snackbar message</Snackbar>
			<Snackbar variant="safe">Success. Snackbar message</Snackbar>
		</div>
	),
});

/** With `onDismiss` the snackbar grows a divider and a close button, and the row with it. */
export const WithDismiss = meta.story({
	render: () => (
		<div style={{ display: "flex", flexDirection: "column", alignItems: "start", gap: 12 }}>
			<Snackbar onDismiss={() => {}}>Info. Snackbar message</Snackbar>
			<Snackbar variant="danger" onDismiss={() => {}}>
				Danger. Snackbar message
			</Snackbar>
			<Snackbar variant="safe" onDismiss={() => {}}>
				Success. Snackbar message
			</Snackbar>
		</div>
	),
});

/** `icon` overrides the variant's own glyph without changing what the snackbar means. */
export const CustomIcon = meta.story({
	render: () => (
		<div style={{ display: "flex", flexDirection: "column", alignItems: "start", gap: 12 }}>
			<Snackbar icon="spinner">Absorbing…</Snackbar>
			<Snackbar icon="absorb" onDismiss={() => {}}>
				Couldn’t work out where to absorb
			</Snackbar>
			<Snackbar variant="safe" icon="commit">
				Changes absorbed into 3 commits
			</Snackbar>
		</div>
	),
});

/** A sentence wider than the space it has ellipsises rather than widening the snackbar. */
export const LongMessage = meta.story({
	render: () => (
		<div style={{ display: "flex", flexDirection: "column", gap: 12, width: 320 }}>
			<Snackbar icon="absorb" onDismiss={() => {}}>
				Couldn’t work out where to absorb these changes, so nothing was moved anywhere at all
			</Snackbar>
			<Snackbar variant="danger" onDismiss={() => {}}>
				The commit could not be created because the workspace has conflicts left in it
			</Snackbar>
		</div>
	),
});
