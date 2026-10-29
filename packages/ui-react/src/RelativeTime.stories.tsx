import preview from "#storybook/preview";
import { RelativeTime } from "./RelativeTime.tsx";

/** Pinned so the wording doesn't drift with the clock between visual checks. */
const now = Date.parse("2026-08-29T12:00:00Z");

const minute = 60_000;
const hour = 60 * minute;
const day = 24 * hour;

const ages = [
	["Seconds", 20_000],
	["Minutes", 26 * minute],
	["Hours", 3 * hour],
	["Days", 4 * day],
	["Months", 70 * day],
	["Years", 800 * day],
] as const;

const meta = preview.meta({
	title: "Content & status/RelativeTime",
	id: "components-relativetime",
	component: RelativeTime,
	args: {
		timestamp: now - 26 * minute,
		now,
		compact: false,
	},
});

/** Hover it for the absolute time. */
export const Default = meta.story({});

/** The tightest reading, for dense rows. */
export const Compact = meta.story({
	args: { compact: true },
});

/** Every unit it steps through, in the full and the compact reading. */
export const Ages = meta.story({
	render: () => (
		<div
			className="text-13"
			style={{
				display: "grid",
				gridTemplateColumns: "repeat(3, auto)",
				gap: "8px 24px",
				width: "fit-content",
			}}
		>
			{ages.map(([label, age]) => (
				<div key={label} style={{ display: "contents" }}>
					<span style={{ color: "var(--text-2)" }}>{label}</span>
					<RelativeTime timestamp={now - age} now={now} />
					<RelativeTime timestamp={now - age} now={now} compact />
				</div>
			))}
		</div>
	),
});
