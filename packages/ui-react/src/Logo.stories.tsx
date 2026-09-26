import preview from "#storybook/preview";
import { Logo, type LogoName } from "./Logo.tsx";

const names = ["github", "gitlab", "bitbucket"] satisfies Array<LogoName>;

const meta = preview.meta({
	component: Logo,
	argTypes: {
		name: { control: "inline-radio", options: names },
	},
	args: {
		name: "github" as LogoName,
		muted: false,
	},
});

export const Default = meta.story({});

/** Each forge in its own colours, and muted: the silhouette for a forge the user hasn't connected. */
export const AllLogos = meta.story({
	render: () => (
		<div
			style={{
				display: "grid",
				gridTemplateColumns: "repeat(3, auto)",
				gap: 16,
				width: "fit-content",
			}}
		>
			{names.map((name) => (
				<Logo key={name} name={name} />
			))}
			{names.map((name) => (
				<Logo key={name} name={name} muted />
			))}
		</div>
	),
});
