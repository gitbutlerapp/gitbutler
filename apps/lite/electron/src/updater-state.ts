import type { ProgressInfo } from "electron-updater";

export type AvailabilitySnapshot =
	| { _tag: "Unavailable" }
	| { _tag: "UpToDate" }
	| { _tag: "Available"; version: string };

export type InstallationStatus =
	| { _tag: "Idle" }
	| {
			_tag: "Downloading";
			version: string;
			progress?: ProgressInfo;
	  }
	| { _tag: "Ready"; version: string }
	| { _tag: "Installing"; version: string };

export const canDownloadUpdate = (state: InstallationStatus): boolean =>
	state._tag !== "Downloading" && state._tag !== "Installing";
