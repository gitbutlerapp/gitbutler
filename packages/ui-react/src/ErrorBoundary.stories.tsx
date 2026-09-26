import preview from "#storybook/preview";
import { useState, type FC } from "react";
import { ErrorBoundary } from "./ErrorBoundary.tsx";

const Broken: FC<{ broken: boolean }> = ({ broken }) => {
	if (broken) throw new Error("Couldn't read the commit graph: object 4f2a9c1 is missing");
	return <p className="text-13">The view rendered.</p>;
};

/** Breaks until Retry, the way a view recovers once whatever it read is back. */
const Recovering: FC = () => {
	const [broken, setBroken] = useState(true);
	return (
		<ErrorBoundary onReset={() => setBroken(false)}>
			<Broken broken={broken} />
		</ErrorBoundary>
	);
};

const meta = preview.meta({
	component: ErrorBoundary,
});

/** What takes the place of a view that threw while rendering: the message, and Retry. */
export const Fallback = meta.story({
	render: () => (
		<ErrorBoundary>
			<Broken broken />
		</ErrorBoundary>
	),
});

/** Retry clears the error and renders the view again. */
export const Retry = meta.story({
	render: () => <Recovering />,
});
