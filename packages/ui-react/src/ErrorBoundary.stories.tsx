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
		<ErrorBoundary title="This view couldn't be shown" onReset={() => setBroken(false)}>
			<Broken broken={broken} />
		</ErrorBoundary>
	);
};

const meta = preview.meta({
	title: "Content & status/ErrorBoundary",
	id: "components-errorboundary",
	component: ErrorBoundary,
});

/** What takes the place of a view that threw while rendering: what couldn't be shown, Retry, and the error to copy. */
export const Default = meta.story({
	render: () => (
		<ErrorBoundary title="This view couldn't be shown">
			<Broken broken />
		</ErrorBoundary>
	),
});

/** Retry clears the error and renders the view again. */
export const Retry = meta.story({
	render: () => <Recovering />,
});
