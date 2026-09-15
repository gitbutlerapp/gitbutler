import { useIsFetching } from "@tanstack/react-query";
import { use, type FC } from "react";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { CheckForUpdatesContext } from "#ui/updater-context.ts";

export const CheckForUpdatesButton: FC = () => {
	const checkForUpdates = use(CheckForUpdatesContext);
	const isCheckingForUpdates = useIsFetching({ queryKey: ["updateCheck"] }) > 0;

	return (
		<button
			type="button"
			className={getButtonClassName({})}
			disabled={isCheckingForUpdates}
			onClick={checkForUpdates}
		>
			Check for updates
			<Icon name={isCheckingForUpdates ? "spinner" : "refresh"} />
		</button>
	);
};
