import preview from "#storybook/preview";
import { Clamped } from "./Clamped.tsx";
import { useEffect, useState } from "react";

const paragraph = (idx: number) => (
	<p key={idx} style={{ margin: "0 0 12px" }}>
		Paragraph {idx + 1}: The quick brown fox jumps over the lazy dog, then circles back to explain
		in considerable detail why it jumped, what the dog thought about it, and how the whole affair
		could have been avoided with better planning.
	</p>
);

const paragraphs = (count: number) => Array.from({ length: count }, (_, idx) => paragraph(idx));

const meta = preview.meta({
	component: Clamped,
	args: {
		maxHeight: "240px",
		// Every story renders its own content; args-driven children are unused.
		children: null,
	},
});

/** Content well over the cap folds behind Show more, with the fade. */
export const Folded = meta.story({
	render: (args) => (
		<div style={{ maxWidth: 480, display: "flex", flexDirection: "column" }}>
			<Clamped {...args}>{paragraphs(12)}</Clamped>
		</div>
	),
});

/** Content shorter than the cap renders with no toggle and no fade. */
export const ShortContent = meta.story({
	render: (args) => (
		<div style={{ maxWidth: 480, display: "flex", flexDirection: "column" }}>
			<Clamped {...args}>{paragraphs(2)}</Clamped>
		</div>
	),
});

/**
 * Content only a hair taller than the cap still folds — expanding reveals
 * almost nothing. Exhibit A for giving the fold trigger some slack over
 * the clamp height.
 */
export const BarelyOverflowing = meta.story({
	render: (args) => (
		<div style={{ maxWidth: 480, display: "flex", flexDirection: "column" }}>
			<Clamped {...args}>{paragraphs(4)}</Clamped>
		</div>
	),
});

const LateGrowth = ({ maxHeight }: { maxHeight: string }) => {
	const [grown, setGrown] = useState(false);
	useEffect(() => {
		const timer = setTimeout(() => setGrown(true), 1500);
		return () => clearTimeout(timer);
	}, []);

	return (
		<div style={{ maxWidth: 480, display: "flex", flexDirection: "column" }}>
			<Clamped maxHeight={maxHeight}>
				{paragraphs(2)}
				{grown ? (
					paragraphs(10)
				) : (
					<p style={{ color: "var(--text-3)" }}>
						…more content arrives in 1.5s (lazy image stand-in)
					</p>
				)}
			</Clamped>
		</div>
	);
};

/**
 * Content that grows after mount (lazy images, async highlighting) still
 * gets its Show more — the ResizeObserver re-measures on growth.
 */
export const GrowsAfterMount = meta.story({
	render: (args) => <LateGrowth maxHeight={args.maxHeight} />,
});
