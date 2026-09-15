import { gt } from "semver";
import { Toast } from "@base-ui/react";
import {
	queryOptions,
	useMutation,
	useQuery,
	useQueryClient,
	useSuspenseQuery,
} from "@tanstack/react-query";
import { type FC, type ReactNode, useEffect, useEffectEvent } from "react";
import { guiSettingsQueryOptions } from "#ui/api/queries.ts";
import { defaultSettings } from "#ui/settings.ts";
import { errorMessageForToast } from "#ui/errors.ts";
import {
	canDownloadUpdate,
	type AvailabilitySnapshot,
	type InstallationStatus,
} from "#electron/updater-state.ts";
import { CheckForUpdatesContext } from "#ui/updater-context.ts";

const updateStatusQueryOptions = queryOptions({
	queryKey: ["updateStatus"],
	queryFn: window.lite.getUpdateStatus,
});

// Share a common ID and title without inheriting any props from previous invocations.
const toastProps = {
	id: "app-update",
	title: "Check for updates",
	description: undefined,
	actionProps: undefined,
	type: undefined,
	timeout: undefined,
};

const megabyteFormat = new Intl.NumberFormat(undefined, {
	style: "unit",
	unit: "megabyte",
	maximumFractionDigits: 1,
});

// Subscribe here so progress ticks update the text without re-adding a dismissed toast.
const DownloadProgress: FC<{ version: string }> = (p) => {
	const { data: progress } = useQuery({
		...updateStatusQueryOptions,
		select: (status) => (status._tag === "Downloading" ? status.progress : undefined),
	});

	return progress === undefined || progress.percent === 100
		? `Preparing update ${p.version}...`
		: `Downloading update ${p.version}... ${Math.floor(progress.percent)}% (${megabyteFormat.format(progress.transferred / 1_000_000)} / ${megabyteFormat.format(progress.total / 1_000_000)})`;
};

type Props = { children: ReactNode };

export const AppUpdater: FC<Props> = (p) => {
	const queryClient = useQueryClient();
	const { add: addToast } = Toast.useToastManager();

	const { data: autoUpdate } = useSuspenseQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => cfg.autoUpdate,
	});

	// Status changes are handled in a watcher, but we still need the eager oneshot for e.g. renderer
	// reload scenarios. Use a local default instead of Suspense so the frontend parent doesn't
	// need to handle updater errors; the query cache still reports failures.
	const { data: updateStatus = { _tag: "Idle" } } = useQuery({
		...updateStatusQueryOptions,
		select: (status) =>
			status._tag === "Downloading" ? { _tag: status._tag, version: status.version } : status,
	});

	const { data: updateAvailability, refetch: refetchUpdateCheck } = useQuery({
		queryKey: ["updateCheck"],
		queryFn: window.lite.checkForUpdates,
		refetchInterval: 60 * 60 * 1000,
		refetchIntervalInBackground: true,
		enabled: (query) =>
			(autoUpdate ?? defaultSettings.autoUpdate) && query.state.data?._tag !== "Unavailable",
	});

	const { mutate: downloadUpdate } = useMutation({
		mutationKey: ["downloadAppUpdate"],
		mutationFn: window.lite.downloadUpdate,
		// Backend handles network connectivity.
		networkMode: "always",
		onError: (error) =>
			addToast({
				...toastProps,
				type: "error",
				description: `Failed to download app update: ${errorMessageForToast(error)}`,
				timeout: 0,
			}),
	});

	const { mutate: installUpdate } = useMutation({
		mutationKey: ["installAppUpdate"],
		mutationFn: window.lite.installUpdate,
		// Does not require network.
		networkMode: "always",
		onError: (error) =>
			addToast({
				...toastProps,
				type: "error",
				description: `Failed to install app update: ${errorMessageForToast(error)}`,
				timeout: 0,
			}),
	});

	// This function is essentially just pattern matching over installation status, update
	// availability, and - if we spiritually include checkManually - whether or not the update check
	// was manual or automatic, which carry different toasting requirements.
	const commonToast = (
		availability: AvailabilitySnapshot | undefined,
		status: InstallationStatus,
	): void => {
		if (status._tag === "Installing")
			return void addToast({ ...toastProps, description: "Restarting...", timeout: 0 });

		if (
			availability?._tag === "Available" &&
			(status._tag === "Idle" || gt(availability.version, status.version))
		) {
			return void addToast({
				...toastProps,
				description: "Update available. Download the update now and restart when you're ready.",
				timeout: 0,
				actionProps: {
					children: `Download ${availability.version}`,
					disabled: !canDownloadUpdate(status),
					onClick: () => downloadUpdate(availability.version),
				},
			});
		}

		switch (status._tag) {
			case "Downloading":
				addToast({
					...toastProps,
					description: <DownloadProgress version={status.version} />,
					timeout: 0,
				});
				break;

			case "Ready":
				addToast({
					...toastProps,
					description: "Update downloaded. Restart now or install on quit.",
					timeout: 0,
					actionProps: {
						children: `Install ${status.version} now`,
						onClick: () => installUpdate(),
					},
				});
				break;
		}
	};

	const checkManually = async (): Promise<void> => {
		addToast({ ...toastProps, description: "Checking for updates...", timeout: 0 });

		const result = await refetchUpdateCheck({ cancelRefetch: false });

		if (result.isError) {
			addToast({
				...toastProps,
				type: "error",
				description: `Failed to check for updates: ${errorMessageForToast(result.error)}`,
			});
		} else if (result.isSuccess) {
			switch (result.data._tag) {
				case "UpToDate":
					addToast({
						...toastProps,
						description: "You are up to date.",
					});
					break;

				case "Unavailable":
					addToast({
						...toastProps,
						type: "error",
						description: "Updates are unavailable in this build.",
					});
					break;

				// For explicit manual checks we display toasts in various additional scenarios as above.
				// Here is where we fall back to the standard cases.
				case "Available":
					commonToast(
						result.data,
						// Read latest status e.g. a download may have completed while the check was in progress.
						queryClient.getQueryData(updateStatusQueryOptions.queryKey) ?? { _tag: "Idle" },
					);
					break;

				default:
					result.data satisfies never;
			}
		}
	};

	const onUpdate = useEffectEvent(() => commonToast(updateAvailability, updateStatus));
	// Keep manual results and errors visible when background state has nothing to announce.
	useEffect(() => {
		if (updateAvailability?._tag === "Available") onUpdate();
	}, [updateAvailability]);
	useEffect(() => {
		if (updateStatus._tag !== "Idle") onUpdate();
	}, [updateStatus]);

	useEffect(
		() =>
			window.lite.onUpdateStatusChange((status) => {
				queryClient.setQueryData(updateStatusQueryOptions.queryKey, status);
			}),
		[queryClient],
	);

	return (
		<CheckForUpdatesContext value={() => void checkManually()}>{p.children}</CheckForUpdatesContext>
	);
};
