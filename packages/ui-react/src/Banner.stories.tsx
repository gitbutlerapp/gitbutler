import preview from "#storybook/preview";
import { Banner, type BannerVariant } from "./Banner.tsx";
import { Button } from "./Button.tsx";

const figma = (nodeId: string) => ({
	design: {
		type: "figma",
		url: `https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=${nodeId}`,
	},
});

const meta = preview.meta({
	title: "Content & status/Banner",
	id: "components-banner",
	component: Banner,
	parameters: figma("2763-6872"),
	argTypes: {
		variant: {
			control: "inline-radio",
			options: ["warn", "danger", "pop", "safe", "gray"] satisfies Array<BannerVariant>,
		},
		icon: { control: "text" },
	},
	args: {
		variant: "warn",
		title: "Live updates paused · may be out of date",
		action: (
			<Button variant="ghost" size="small">
				Retry
			</Button>
		),
	},
	decorators: [
		(Story) => (
			<div style={{ width: 360 }}>
				<Story />
			</div>
		),
	],
});

/** The banner as the design states it: a glyph, a title, and the fix. */
export const Default = meta.story({
	args: { variant: "warn", title: "Live updates paused · may be out of date" },
});

/** Five variants, chosen by meaning. Gray carries no verdict, so its title keeps the body colour. */
export const AllVariants = meta.story({
	render: () => (
		<div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
			{(["warn", "danger", "pop", "safe", "gray"] satisfies Array<BannerVariant>).map((variant) => (
				<Banner
					key={variant}
					variant={variant}
					title="Live updates paused · may be out of date"
					action={
						<Button variant="ghost" size="small">
							Retry
						</Button>
					}
				/>
			))}
		</div>
	),
});

/** The live channel dropped and hasn't come back: the page still works, but may be stale. */
export const LiveUpdatesPaused = meta.story({
	parameters: figma("2773-10447"),
	render: () => (
		<Banner
			variant="warn"
			title="Live updates paused · may be out of date"
			action={
				<Button variant="ghost" size="small">
					Retry
				</Button>
			}
		/>
	),
});

/** A machine stopped sending heartbeats; the description says what the user can do about it. */
export const MachineOffline = meta.story({
	parameters: figma("2773-10473"),
	render: () => (
		<Banner
			variant="gray"
			title="pavel-macbook-pro-2 is offline"
			action={
				<Button variant="ghost" size="small">
					Copy command
				</Button>
			}
		>
			Last seen 3d ago. Wake it up, or run but mesh service restart there.
		</Banner>
	),
});

/** The server won't answer. `danger` announces itself to screen readers as an alert. */
export const Unreachable = meta.story({
	parameters: figma("2773-10497"),
	render: () => (
		<Banner
			variant="danger"
			title="Couldn't reach your mesh"
			action={
				<Button variant="ghost" size="small">
					Try again
				</Button>
			}
		>
			The server answered “502 Bad Gateway”. Trying again every few seconds.
		</Banner>
	),
});

/** Something new happened elsewhere that the user can act on. */
export const MovedElsewhere = meta.story({
	parameters: figma("2773-10521"),
	render: () => (
		<Banner
			variant="pop"
			title="review-cards moved on pavel-mini"
			action={
				<Button variant="ghost" size="small">
					Show
				</Button>
			}
		>
			3 new commits since you opened it.
		</Banner>
	),
});

/** With nothing to do about it, the banner drops the action; `icon={false}` drops the glyph. */
export const WithoutActionOrIcon = meta.story({
	parameters: figma("2773-10545"),
	render: () => (
		<div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
			<Banner variant="gray" title="This branch is read-only on this machine" />
			<Banner variant="warn" icon={false} title="Updates arrive every minute while offline" />
		</div>
	),
});
