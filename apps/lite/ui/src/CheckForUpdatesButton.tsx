import { useIsFetching } from "@tanstack/react-query";
import { use, type FC } from "react";
import { Button } from "@gitbutler/ui-react/Button.tsx";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { CheckForUpdatesContext } from "#ui/updater-context.ts";

export const CheckForUpdatesButton: FC = () => {
	const checkForUpdates = use(CheckForUpdatesContext);
	const isCheckingForUpdates = useIsFetching({ queryKey: ["updateCheck"] }) > 0;

	return (
		<Button disabled={isCheckingForUpdates} onClick={checkForUpdates}>
			Check for updates
			<Icon name={isCheckingForUpdates ? "spinner" : "refresh"} />
		</Button>
	);
};
