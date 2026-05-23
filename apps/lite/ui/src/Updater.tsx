import { AlertDialog } from "@base-ui/react";
import { useEffect, useState } from "react";
import type { FC } from "react";
import { Button } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import uiStyles from "#ui/components/ui.module.css";

export const Updater: FC = () => {
	const [version, setVersion] = useState<string | null>(null);

	useEffect(() => {
		const unsubscribeDownloaded = window.lite.onUpdateDownloaded((info) => {
			setVersion(info.version);
		});

		return () => {
			unsubscribeDownloaded();
		};
	}, []);

	return (
		<AlertDialog.Root
			open={version !== null}
			onOpenChange={() => {
				// We do not allow dismissing this dialog at this time
			}}
		>
			<AlertDialog.Portal>
				<AlertDialog.Backdrop />
				<AlertDialog.Popup className={classes(uiStyles.popup, uiStyles.dialogPopup)}>
					<AlertDialog.Title>Update {version} downloaded</AlertDialog.Title>
					<AlertDialog.Description>
						Restart to update. You don't have a choice :)
					</AlertDialog.Description>
					<div>
						<Button type="button" onClick={() => void window.lite.quitAndInstallUpdate()}>
							Restart and install
						</Button>
					</div>
				</AlertDialog.Popup>
			</AlertDialog.Portal>
		</AlertDialog.Root>
	);
};
